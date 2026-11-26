//! RenderOutput jobs run in the runtime (#8826): `CaptureImage` through
//! render-wgpu's `capture_image`, `ExportSceneGlb` through render-export's `export_glb`. No
//! browser is involved, so unattended batches need no Chromium.
//!
//! A worker thread runs jobs in settle order, away from the product call path.
//! Each result goes back through the job's completion, and the product reads
//! it from its next callback on. Image jobs draw with a renderer of their own
//! on the device the runtime already draws with, the stream's or the
//! window's, built on the first image job. Only a runtime that draws nothing
//! (an exercise or probe run) gets a device for them, made on the first image
//! job and kept until the worker stops; a GLB export needs none.
//!
//! One device per process matters in debug builds: Ubuntu's Vulkan loader
//! (1.4.341, KhronosGroup/Vulkan-Loader#1863) can read freed memory when one
//! thread makes or drops a device while another names objects, which wgpu
//! does in debug builds.

use std::borrow::Cow;
use std::sync::mpsc::{self, Sender};

use csharp_engine_services::{RenderOutputResources, RenderOutputWork};
use render_export::export_glb;
use render_host_contracts::RenderOutputOperation;
use render_wgpu::{Gpu, Renderer, RendererOptions, ResourceSource};

pub(crate) struct OutputExecutor {
    jobs: Sender<RenderOutputWork>,
}

impl OutputExecutor {
    /// `gpu` is the device the runtime draws with, if it draws.
    pub(crate) fn start(options: RendererOptions, gpu: Option<Gpu>) -> std::io::Result<Self> {
        let (jobs, receiver) = mpsc::channel::<RenderOutputWork>();
        std::thread::Builder::new()
            .name("rusty-render-output".to_owned())
            .spawn(move || {
                let mut gpu = gpu;
                let mut renderer: Option<Result<Renderer, String>> = None;
                for work in receiver {
                    let resources = JobResources(&work.resources);
                    let result = match work.job.operation {
                        RenderOutputOperation::Glb { .. } => export_glb(&work.job, &resources),
                        RenderOutputOperation::Image { .. } => renderer
                            .get_or_insert_with(|| {
                                gpu.take()
                                    .map_or_else(
                                        || Gpu::headless_for(options.settings().ray_traced()),
                                        Ok,
                                    )
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
struct JobResources<'a>(&'a RenderOutputResources);

impl ResourceSource for JobResources<'_> {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0.bytes(identity).map(Cow::Borrowed)
    }
}
