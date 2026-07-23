# Rusty Engine agent guidance

This repository is an external architecture experiment. Current Asha architecture is evidence and a donor source, not a compatibility constraint.

- Keep the runtime object-centric: entities, components, relationships, responsible services/behaviors, and typed facts must be easy to trace.
- Components are mostly data. Do not add implicit update callbacks, ambient subscriptions, service location, or renderer/I/O behavior to components.
- The Rust world kernel owns reusable engine invariants and atomic capability mutation.
- Direct Rust services and trusted executable TypeScript are competing game-code hosts until measured evidence selects them.
- TypeScript game code receives bounded read-only batches and returns bounded command/state/schedule batches. It never receives mutable Rust state or renderer objects.
- Do not add a universal gameplay AST, behavior graph, Gameplay Fabric compatibility, Studio layer, replay certification, or broad governance framework during the spike.
- Keep scheduling explicit and durable. Never persist callbacks or JavaScript closures.
- Prefer sibling references to stable Asha donor crates during the experiment. Record every donor in `docs/donor-provenance.md`.
- Keep crates/packages coarse and independently meaningful.
- Success is measured by behavior locality, explainability, atomicity, persistence, bridge cost, edit-to-run time, and real product behavior.
