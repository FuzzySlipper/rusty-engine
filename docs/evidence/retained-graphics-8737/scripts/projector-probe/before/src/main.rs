// Before (64a9b164): every frame republishes the complete snapshot.
#[path = "../../scenario.rs"]
mod scenario;

use render_projection::{RuntimeAppearanceFact, RuntimeAppearanceProjector};
use scenario::*;

fn facts(count: u64, frame: usize) -> Vec<RuntimeAppearanceFact> {
    (1..=count)
        .map(|id| {
            let (appearance, transform, visible, layer) = object(id, frame);
            RuntimeAppearanceFact {
                object_id: id,
                parent_object_id: None,
                appearance: appearance.to_owned(),
                transform,
                visible,
                layer,
            }
        })
        .collect()
}

fn main() {
    for count in [1_000_u64, 5_000, 20_000] {
        let mut projector = RuntimeAppearanceProjector::new(catalog());
        projector.project(&facts(count, 0)).unwrap();
        // The product builds its facts outside the measured call.
        let prepared: Vec<_> = (0..=FRAMES).map(|frame| facts(count, frame)).collect();
        let (p50, p95, operations) = measure(|frame| {
            projector.project(&prepared[frame]).unwrap().frame.ops.len()
        });
        println!(
            "{{\"side\":\"before\",\"path\":\"snapshot\",\"objects\":{count},\"moving\":{MOVING},\"usP50\":{p50},\"usP95\":{p95},\"operations\":{operations}}}"
        );
    }
}
