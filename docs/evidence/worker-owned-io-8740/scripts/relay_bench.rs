// Scratch bench for #8740: worker->shell relay cost versus direct SSE encoding.
use super::*;
use std::time::Instant;

fn moving_frame(objects: u64, tick: u64) -> render_model::RenderFrameDiff {
    render_model::RenderFrameDiff {
        ops: (0..objects)
            .map(|handle| render_model::RenderDiff::Update {
                handle: render_model::RenderHandle::new(handle + 1),
                transform: Some(render_model::Transform {
                    translation: [handle as f32 * 0.37, (tick as f32).sin(), -(handle as f32) * 1.13],
                    ..render_model::Transform::IDENTITY
                }),
                material: None,
                visible: None,
                metadata: None,
            })
            .collect(),
        ..render_model::RenderFrameDiff::new()
    }
}

#[test]
#[ignore]
fn bench_8740_relay_cost() {
    for objects in [100u64, 1_000, 10_000] {
        let ticks = 60u64;
        let (mut direct, mut worker_encode, mut shell_decode, mut bytes) = (0u128, 0u128, 0u128, 0usize);
        let mut bus = OutputBus { active_binding: Some(binding()), ..OutputBus::default() };
        let mut relay_bus = OutputBus { active_binding: Some(binding()), ..OutputBus::default() };
        for tick in 0..ticks {
            let output = crate::model::ProductDevRuntimeOutput::frame(&moving_frame(objects, tick)).unwrap();
            // In-process: one SSE encode.
            let started = Instant::now();
            push_outputs_staged(&mut bus, vec![output.clone()]).unwrap();
            direct += started.elapsed().as_nanos();
            // Worker: typed -> Value -> bytes on the worker pipe.
            let started = Instant::now();
            let value = output.to_worker_value().unwrap();
            let wire = serde_json::to_vec(&serde_json::json!({"kind":"outputs","outputs":[value]})).unwrap();
            worker_encode += started.elapsed().as_nanos();
            bytes = wire.len();
            // Shell: bytes -> Value -> typed -> SSE encode.
            let started = Instant::now();
            let mut event: serde_json::Value = serde_json::from_slice(&wire).unwrap();
            let value = event["outputs"].as_array_mut().unwrap().remove(0);
            let decoded = crate::model::ProductDevRuntimeOutput::from_worker_value(value).unwrap();
            push_outputs_staged(&mut relay_bus, vec![decoded]).unwrap();
            shell_decode += started.elapsed().as_nanos();
        }
        let per = |ns: u128| format!("{:.3}ms", ns as f64 / ticks as f64 / 1e6);
        eprintln!(
            "BENCH objects={objects} wire_bytes={bytes} direct_sse={} worker_encode={} shell_decode_reencode={}",
            per(direct), per(worker_encode), per(shell_decode)
        );
    }
}
