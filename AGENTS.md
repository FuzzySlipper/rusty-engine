# Rusty Engine agent guidance

This repository is the standalone canonical Rusty Engine provider. Asha architecture is historical evidence and a donor source, not a compatibility constraint or operational dependency. The loading-bay product is owned by the external `rusty-engine-demo` consumer; Engine must not depend on or inspect a sibling demo checkout during ordinary work.

The implemented architecture is documented in [docs/design.md](docs/design.md). Keep that document current when changing authority, execution order, persistence, or presentation boundaries.

Rust source organization follows the lightweight house rules in [docs/rust-style.md](docs/rust-style.md). They are navigation and ownership guidance, not a new governance layer.

- Keep the provider object-centric: entity capabilities, spatial authority, responsible services/systems, and typed facts must be easy to trace.
- Components are mostly data. Do not add implicit update callbacks, ambient subscriptions, service location, or renderer/I/O behavior to components.
- Downstream Rust owns live authoritative gameplay state and substantial game logic through direct named services. Engine owns only reusable mechanisms proved by concrete consumers.
- Ordinary Engine work has no Node, TypeScript, browser, renderer, or demo dependency. Those concerns belong downstream or in a separately isolated tool workspace with its own explicit gate.
- Rust `entity-state` owns reusable entity invariants and atomic capability mutation. Do not turn its command batch into a universal route for ordinary service-owned state.
- Do not add a universal gameplay AST, behavior graph, Gameplay Fabric compatibility, Studio layer, replay certification, or broad governance framework without a concrete consumer and explicit architecture decision.
- Engine does not own a universal game scheduler. Any reusable scheduling primitive must be justified by multiple concrete consumers; never persist callbacks or language closures.
- Before defining a migration milestone or selecting an Asha crate, consult the portability report linked from `docs/donor-provenance.md`; treat it as donor-triage evidence, then re-audit the concrete dependency closure and consumer.
- Do not add operational sibling-checkout dependencies. Internalize only a bounded, audited donor closure and record every transfer or adaptation in `docs/donor-provenance.md`.
- Keep crates/packages coarse and independently meaningful.
- Success is measured by mechanism locality, explainability, atomicity, focused provider evidence, bounded dependencies, and standalone operation. Real product behavior is proved in downstream consumers.
