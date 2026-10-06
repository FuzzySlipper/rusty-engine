//! EXPLORE #9513: mesh dumped coarse lattices on the GPU and report the cost.
//!   cargo run --release -p render-wgpu --example gpu_dc -- <dump-dir> [batch] [repeats]

use render_wgpu::gpu_dc::{GpuDualContouring, Lattice};
use render_wgpu::Gpu;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("dump directory");
    let batch: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(64);
    let repeats: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(5);
    let mut lattices: Vec<Lattice> = std::fs::read_dir(&dir)
        .expect("dump directory")
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "lat"))
        .filter_map(|entry| Lattice::read(&std::fs::read(entry.path()).ok()?))
        .collect();
    lattices.sort_by_key(|l| std::cmp::Reverse(l.cpu_triangles));
    let gpu = Gpu::headless().expect("adapter");
    let info = gpu.adapter_summary();
    println!("adapter: {} ({})", info.name, info.backend);
    println!("lattices: {} (dims {:?})", lattices.len(), lattices.first().map(|l| l.dims));
    let dc = GpuDualContouring::new(&gpu);
    // Warm up pipelines.
    let _ = dc.mesh(&gpu, &lattices[..lattices.len().min(batch)]);
    println!("{:>6} {:>8} {:>10} {:>10} {:>8} {:>12} {:>12} {:>10} {:>10} {:>6}",
        "chunks", "upload KB", "cells ms", "edges ms", "wall ms", "gpu tris", "cpu tris", "gpu verts", "cpu verts", "ovfl");
    for chunk_batch in lattices.chunks(batch).take(8) {
        let mut best: Option<render_wgpu::gpu_dc::Report> = None;
        for _ in 0..repeats {
            let report = dc.mesh(&gpu, chunk_batch);
            let better = best.as_ref().is_none_or(|b| report.wall_ms < b.wall_ms);
            if better {
                best = Some(report);
            }
        }
        let r = best.expect("a report");
        println!("{:>6} {:>8.1} {:>10.3} {:>10.3} {:>8.2} {:>12} {:>12} {:>10} {:>10} {:>6}",
            r.chunks, r.upload_bytes as f64 / 1024.0, r.cells_gpu_ms, r.edges_gpu_ms, r.wall_ms,
            r.gpu_triangles, r.cpu_triangles, r.gpu_vertices, r.cpu_vertices, r.overflowed);
    }
}
