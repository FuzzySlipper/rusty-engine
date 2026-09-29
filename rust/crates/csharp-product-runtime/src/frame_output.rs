//! The world rendered in this process and streamed to the browser shell.
//!
//! `RUSTY_RENDER_OUTPUT=stream` starts a `render-wgpu` renderer when the
//! runtime loads. Each committed call's renderer publications are applied to
//! it as they are committed, and the host serves the frames it draws at
//! `/__rusty/product/runtime/frames`. The browser shell then shows those
//! frames under the product UI instead of realizing the world with Three.
//! The publications still reach the browser; its stream surface ignores the
//! world and realizes only audio and video.
//!
//! Animation facts reach the Engine from this renderer, through the same
//! realization feedback the browser reports.
//! `RUSTY_RENDER_STREAM_FORMAT=rgba` sends raw frames instead of JPEG, to
//! measure what the encoder saves.

use std::borrow::Cow;
use std::sync::Arc;

use csharp_engine_services::{AnimationRealizationFact, EngineServiceSet};
use product_dev_host::ProductDevFrameStream;
use render_stream::{
    AnimationFact, FrameStreamer, RendererOptions, ResourceSource, StreamFormat, StreamStats,
};
use runtime_publication::RuntimePublication;

use crate::CsharpProductRuntimeError;

pub(crate) const RENDER_OUTPUT_ENV: &str = "RUSTY_RENDER_OUTPUT";
const RENDER_OUTPUT_STREAM: &str = "stream";
const STREAM_FORMAT_ENV: &str = "RUSTY_RENDER_STREAM_FORMAT";
/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;

/// Whether this process renders the world and streams it. An unset variable
/// keeps browser realization; an unknown value is an error rather than a
/// silent fallback.
pub(crate) fn stream_selected() -> Result<bool, CsharpProductRuntimeError> {
    match std::env::var_os(RENDER_OUTPUT_ENV) {
        None => Ok(false),
        Some(value) if value == RENDER_OUTPUT_STREAM => Ok(true),
        Some(_) => Err(CsharpProductRuntimeError::new(
            "CSHARP_RENDER_OUTPUT",
            format!("{RENDER_OUTPUT_ENV} must be `{RENDER_OUTPUT_STREAM}` when set"),
        )),
    }
}

pub(crate) struct FrameOutput {
    streamer: FrameStreamer,
    frames: Arc<ProductDevFrameStream>,
    next_fact_id: u64,
}

impl FrameOutput {
    pub(crate) fn from_environment(
        options: RendererOptions,
    ) -> Result<Option<Self>, CsharpProductRuntimeError> {
        if !stream_selected()? {
            return Ok(None);
        }
        let format = match std::env::var(STREAM_FORMAT_ENV).as_deref() {
            Err(_) | Ok("jpeg") => StreamFormat::Jpeg,
            Ok("rgba") => StreamFormat::Rgba8,
            Ok(_) => {
                return Err(CsharpProductRuntimeError::new(
                    "CSHARP_RENDER_OUTPUT",
                    format!("{STREAM_FORMAT_ENV} must be `jpeg` or `rgba` when set"),
                ))
            }
        };
        let frames = ProductDevFrameStream::new();
        let streamer = FrameStreamer::start(options, format, Arc::clone(&frames))
            .map_err(|message| CsharpProductRuntimeError::new("CSHARP_RENDER_OUTPUT", message))?;
        Ok(Some(Self {
            streamer,
            frames,
            next_fact_id: 1,
        }))
    }

    pub(crate) fn frames(&self) -> Arc<ProductDevFrameStream> {
        Arc::clone(&self.frames)
    }

    /// Applies a committed call's renderer publications.
    pub(crate) fn realize(&self, services: &EngineServiceSet, outputs: &[RuntimePublication]) {
        self.streamer.apply(
            outputs,
            &EngineResources(services),
            &|entity| services.entity_world_position(entity),
            services.presentation_elapsed_seconds(),
        );
    }

    /// Rebuilds the renderer from the committed world, after a call's
    /// renderer work was lost or the world was replaced.
    pub(crate) fn rebaseline(
        &self,
        services: &EngineServiceSet,
        baseline: &[RuntimePublication],
    ) {
        self.streamer.rebaseline(
            baseline,
            &EngineResources(services),
            &|entity| services.entity_world_position(entity),
            services.presentation_elapsed_seconds(),
        );
    }

    pub(crate) fn follow_simulation(&self, held: bool, step: u64) {
        self.streamer.set_simulation(held, step);
    }

    /// Reports what the drawn frames observed since the last call. Call
    /// between product calls.
    pub(crate) fn report(&mut self, services: &mut EngineServiceSet) {
        let facts = self.streamer.take_animation_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk
                .iter()
                .map(|fact| self.engine_fact(fact.clone()))
                .collect();
            services.ingest_animation_realization_feedback(false, 0, facts);
        }
    }

    /// What the recent streamed frames cost, for `engine.renderer.*`.
    pub(crate) fn stats_json(&self) -> serde_json::Value {
        let StreamStats {
            adapter,
            frames,
            frames_per_second,
            render_ms,
            readback_ms,
            encode_ms,
            bytes_per_frame,
            bytes_per_second,
            skipped_ops,
            last_skip,
        } = self.streamer.stats();
        serde_json::json!({
            "adapter": adapter,
            "viewerSize": self.frames.wanted_size(),
            "recentFrames": frames,
            "framesPerSecond": frames_per_second,
            "medianMs": { "render": render_ms, "readback": readback_ms, "encode": encode_ms },
            "medianBytesPerFrame": bytes_per_frame,
            "bytesPerSecond": bytes_per_second,
            "skippedOps": skipped_ops,
            "lastSkip": last_skip,
        })
    }

    fn engine_fact(&mut self, fact: AnimationFact) -> AnimationRealizationFact {
        let fact_id = self.next_fact_id;
        self.next_fact_id += 1;
        match fact {
            AnimationFact::NaturalCompletion {
                object_id,
                generation,
                clip,
            } => AnimationRealizationFact::NaturalCompletion {
                fact_id,
                object_id,
                generation,
                clip,
            },
            AnimationFact::MeshInspection {
                object_id,
                generation,
                request,
                bounds,
            } => AnimationRealizationFact::MeshInspection {
                fact_id,
                object_id,
                generation,
                request,
                bounds_min: bounds.map_or([0.0; 3], |(min, _)| min),
                bounds_max: bounds.map_or([0.0; 3], |(_, max)| max),
                has_bounds: bounds.is_some(),
                voxel_normal_meshes: 0,
            },
        }
    }
}

struct EngineResources<'a>(&'a EngineServiceSet);

impl ResourceSource for EngineResources<'_> {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .borrowed_renderer_resource(identity)
            .map(|resource| Cow::Borrowed(resource.bytes()))
    }
}
