// After: the same snapshot through the adapter, and only the moved objects
// through `apply`.
#[path = "../../scenario.rs"]
mod scenario;

use render_projection::{RuntimeAppearanceFact, RuntimeAppearanceProjector};
use scenario::*;

fn facts(ids: impl Iterator<Item = u64>, frame: usize) -> Vec<RuntimeAppearanceFact<'static>> {
    ids.map(|id| {
        let (appearance, transform, visible, layer) = object(id, frame);
        RuntimeAppearanceFact {
            object_id: id,
            parent_object_id: None,
            appearance,
            transform,
            visible,
            layer,
            joint: None,
        }
    })
    .collect()
}

fn main() {
    for count in [1_000_u64, 5_000, 20_000] {
        for path in ["snapshot", "changes"] {
            let mut projector = RuntimeAppearanceProjector::new(catalog());
            projector.project(&facts(1..=count, 0)).unwrap();
            let prepared: Vec<_> = (0..=FRAMES)
                .map(|frame| match path {
                    "snapshot" => facts(1..=count, frame),
                    _ => facts(1..=MOVING, frame),
                })
                .collect();
            let (p50, p95, operations) = measure(|frame| {
                let facts = &prepared[frame];
                match path {
                    "snapshot" => projector.project(facts),
                    _ => projector.apply(facts, &[]),
                }
                .unwrap()
                .ops
                .len()
            });
            println!(
                "{{\"side\":\"after\",\"path\":\"{path}\",\"objects\":{count},\"moving\":{MOVING},\"usP50\":{p50},\"usP95\":{p95},\"operations\":{operations}}}"
            );
        }
    }
}
