# Rust source organization

This is a lightweight house style for the successor experiment. Its purpose is to keep gameplay
code easy to locate, change, and explain. It is not a formatting standard, a framework, or a reason
to reproduce Asha's architecture governance.

The central rule is:

> One primary behavior owner or cohesive type family per file. Related configuration, state, facts,
> errors, validation, and small helpers stay with that owner.

This is intentionally not one type per file. A gameplay concept is usually clearest when its small
family of data and behavior can be read together.

## Organize by feature ownership

Prefer modules named for a gameplay or runtime responsibility:

```text
src/
  lib.rs
  runtime.rs
  session.rs
  combat.rs
  combat/
    model.rs
    service.rs
  navigation.rs
  navigation/
    model.rs
    system.rs
```

Start with one feature file. Add child files when the feature has multiple independently substantial
parts. Do not create an otherwise empty directory and `mod.rs` merely to anticipate growth.

Avoid catch-all modules such as `model.rs`, `services.rs`, `types.rs`, `common.rs`, or `utils.rs` at
the crate root. A locally scoped `combat/model.rs` is useful because its owner is unambiguous; a
crate-wide `model.rs` is usually an invitation to unrelated accretion.

Crate roots stay thin: crate-level documentation, module declarations, and selective public
re-exports only. Implementations and miscellaneous helpers belong with their owner.

## Use ownership vocabulary consistently

Names should reveal what a type is responsible for:

| Suffix | Meaning |
| --- | --- |
| `Config` | Authored or admitted immutable configuration. |
| `State` | Durable mutable state owned by one concept. |
| `Component` | The cohesive config/state data attached to an entity. |
| `View` | A read-only projection assembled for a caller. |
| `Service` | An explicit operation owner invoked in response to a request or fact. |
| `System` | An explicit phase owner that processes a selected set of entities. |
| `Fact` | A typed outcome committed by an operation or phase. |
| `Receipt` | The complete typed result returned by an operation. |
| `Event` | A typed committed occurrence that has an explicit downstream route. |
| `Snapshot` | Durable serialized state used to reconstruct a session. |

These suffixes document roles; they are not base classes or required traits. Use a simpler name when
it communicates ownership better.

## Keep authority direct

- Components are mostly data. Validation and a small derived-value helper may live beside them, but
  components do not poll, subscribe, locate services, perform I/O, or update themselves.
- Mutating fields should be private to the smallest practical module. Responsible services and
  systems expose explicit operations and return typed facts or receipts.
- `Service` is for request-shaped behavior such as an attack or door transition. `System` is for a
  centrally invoked phase such as navigation or kinematic motion. Neither implies a framework.
- Cross-domain ordering remains visible in a short runtime or host method. Do not replace direct
  calls with an ambient event bus, reaction registry, component callback, or generic pipeline.
- Events may carry meaningful consequences, but they remain typed and their consumers are explicit.
  Facts that need no cross-domain route should remain direct return values.
- Snapshot reconstruction may be somewhat aggregate because it owns cross-domain invariants. Keep
  the rebuild order explicit and move only genuinely feature-local records or conversions into child
  modules.

## Spend abstractions only when proved necessary

Prefer concrete types and direct calls. Add a trait when there is a real boundary that must be
substituted or more than one meaningful implementation. Add a generic only when it removes repeated
logic without hiding gameplay ownership.

Do not introduce a service locator, dependency-injection container, plugin registry, universal
handler, prelude, gameplay AST, behavior graph, or macro-generated routing as a default convenience.
Names such as `Manager`, `Handler`, and `Util` need a more precise responsibility before they enter
the source tree.

Small private helpers are welcome beside their owner. Promote a helper only when multiple real
callers establish the shared concept.

## Keep dependency direction legible

Visibility starts private, expands to `pub(crate)` for actual crate collaboration, and becomes `pub`
only for an intentional crate API. Re-export only the public concepts callers should name; do not
mirror every internal module through the crate root.

Gameplay feature code may depend on entity state, spatial capabilities, and other established lower
layers. Runtime orchestration may depend on gameplay features. Avoid making a feature depend on a
browser host, presentation shell, or content generator.

Possible downstream reuse is a locality test, not a design target during this spike. Game-specific
concepts should have an obvious module and limited dependencies so they can be evaluated or moved
later. Do not add extraction traits, downstream facades, plugin APIs, registries, or compatibility
layers before a real downstream use establishes their shape.

## Use file size as a review signal

Line counts are prompts for judgment, never CI gates:

- Roughly 150 to 450 lines is a comfortable target for a substantial module.
- Around 600 lines, review whether more than one behavior owner or change reason has accumulated.
- Around 900 lines, keep the file only with a clear cohesion reason, such as explicit aggregate
  reconstruction that is easier to verify in one place.
- A short file is fine when its responsibility is complete. Do not split merely to hit a target.

The failure modes on either side are real: huge files encourage distant invariants and endless
accretion, while one-type files create navigation churn and re-export mazes.

## Review source changes with four questions

1. Can a reader find the responsible feature from the type or operation name?
2. Is mutable authority owned by one visible service, system, session, or runtime method?
3. Does the change keep the local behavioral story together without creating a catch-all file?
4. Did any new abstraction solve an observed need, or only anticipate a possible future one?

If those answers are clear, prefer the straightforward implementation over a more abstract pattern.
