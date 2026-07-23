# Rusty Engine agent guidance

This repository is an external architecture experiment. Current Asha architecture is evidence and a donor source, not a compatibility constraint.

- Keep the runtime object-centric: entities, components, relationships, responsible services/behaviors, and typed facts must be easy to trace.
- Components are mostly data. Do not add implicit update callbacks, ambient subscriptions, service location, or renderer/I/O behavior to components.
- Rust owns live authoritative gameplay state and substantial game logic through direct named services.
- TypeScript is an authoring/tooling language for composing strict project content before session admission, plus the eventual presentation shell. Do not add a second runtime authority or per-gameplay bridge without explicitly reopening the archived experiment.
- The Rust world kernel owns reusable engine invariants and atomic capability mutation. Do not turn its command batch into a universal route for ordinary service-owned state.
- Do not add a universal gameplay AST, behavior graph, Gameplay Fabric compatibility, Studio layer, replay certification, or broad governance framework during the spike.
- Keep scheduling explicit and durable. Never persist callbacks or JavaScript closures.
- Prefer sibling references to stable Asha donor crates during the experiment. Record every donor in `docs/donor-provenance.md`.
- Keep crates/packages coarse and independently meaningful.
- Success is measured by behavior locality, explainability, atomicity, persistence, edit-to-run time, change amplification, and real product behavior.
