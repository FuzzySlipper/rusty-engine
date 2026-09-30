//! RenderOutput jobs run in the runtime (#8826): `CaptureImage` through
//! render-wgpu's `capture_image`, `ExportSceneGlb` through render-export's `export_glb`. No
//! browser is involved, so unattended batches need no Chromium.
//!
//! A worker thread runs jobs in settle order, away from the product call path.
//! Each result goes back through the job's completion, and the product reads
//! it from its next callback on. The worker creates its wgpu device on the
//! first image job; a GLB export needs none.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};

use csharp_engine_services::{CsharpRenderResource, RenderOutputWork};
use render_export::export_glb;
use render_host_contracts::RenderOutputOperation;
use render_wgpu::{Gpu, Renderer, RendererOptions, ResourceSource};

pub(crate) struct OutputExecutor {
    jobs: Sender<RenderOutputWork>,
}

impl OutputExecutor {
    pub(crate) fn start(options: RendererOptions) -> std::io::Result<Self> {
        let (jobs, receiver) = mpsc::channel::<RenderOutputWork>();
        std::thread::Builder::new()
            .name("rusty-render-output".to_owned())
            .spawn(move || {
                let mut renderer: Option<Result<Renderer, String>> = None;
                for work in receiver {
                    let resources = JobResources::new(&work.resources);
                    let result = match work.job.operation {
                        RenderOutputOperation::Glb { .. } => export_glb(&work.job, &resources),
                        RenderOutputOperation::Image { .. } => renderer
                            .get_or_insert_with(|| {
                                Gpu::headless()
                                    .map(|gpu| Renderer::new(&gpu, options))
                                    .map_err(|error| format!("capture: no wgpu adapter: {error}"))
                            })
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|renderer| renderer.capture_image(&work.job, &resources)),
                    };
                    work.complete(result);
                }
            })?;
        Ok(Self { jobs })
    }

    /// Queue a call's settled jobs.
    pub(crate) fn submit(&self, work: Vec<RenderOutputWork>) {
        for job in work {
            // A stopped worker drops the job, which fails it.
            let _ = self.jobs.send(job);
        }
    }
}

/// The resources a job's frozen frame reads, by identity.
struct JobResources<'a>(HashMap<&'a str, &'a CsharpRenderResource>);

impl<'a> JobResources<'a> {
    fn new(resources: &'a [CsharpRenderResource]) -> Self {
        Self(
            resources
                .iter()
                .map(|resource| (resource.identity(), resource))
                .collect(),
        )
    }
}

impl ResourceSource for JobResources<'_> {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(identity)
            .map(|resource| Cow::Borrowed(resource.bytes()))
    }
}
