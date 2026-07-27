# Entity state and state machines

## Purpose

Route work involving reusable entity identity, typed components, transforms,
relationships, activation, snapshots, and explicit state-machine instances.

## Owns

- `entity-state`: entity definitions and instances, one instance-owned typed
  component store, stable component registration, relationship facts,
  transforms, activation, views, snapshots, and atomic component mutation.
- `state-machine`: named state-machine definitions and instance transitions
  over entity identifiers.
- Validation and typed errors at those mutation boundaries.

## Does not own

- Ordinary service-owned spatial, collision, navigation, asset, or presentation
  state.
- A universal command bus, gameplay behavior graph, scheduler, or replay
  runtime.
- Game-specific rules or orchestration.

## Primary paths

- [`entity-state/src/lib.rs`](../../rust/crates/entity-state/src/lib.rs)
- [`entity-state/src/component.rs`](../../rust/crates/entity-state/src/component.rs)
- [`entity-state/src/component/registration.rs`](../../rust/crates/entity-state/src/component/registration.rs)
- [`entity-state/src/components.rs`](../../rust/crates/entity-state/src/components.rs)
- [`entity-state/src/authoring.rs`](../../rust/crates/entity-state/src/authoring.rs)
- [`entity-state/src/command.rs`](../../rust/crates/entity-state/src/command.rs)
- [`entity-state/src/relationship.rs`](../../rust/crates/entity-state/src/relationship.rs)
- [`entity-state/src/snapshot.rs`](../../rust/crates/entity-state/src/snapshot.rs)
- [`state-machine/src/lib.rs`](../../rust/crates/state-machine/src/lib.rs)
- [Canonical design](../design.md)

## Public downstream surfaces

- The crate roots re-export the supported data, mutation, snapshot, and
  transition vocabulary.
- Downstream component types implement `EntityComponent`, receive a stable
  `ComponentTypeId`, and are registered explicitly on a `ComponentRegistry` or
  `EntityState`. The store supplies typed read/has/iteration while
  `EntityAuthoringService` owns attach/replace/remove guarded by an
  instance-local `ComponentRevision` for the exact entity/component slot.
- Downstream Rust composes these mechanisms directly with named game services.
- Serialized entity snapshots are durable facts only where the owning consumer
  chooses to persist them.
- Runtime-only registrations are omitted from snapshots. Durable downstream
  components require an explicit codec identity/version and the same registry
  during decode; built-in schema-3 fields retain their established JSON shape.

## Downstream extension pattern

A downstream crate declares inert data, registers its authored identity on the
specific registry used to build the state, and keeps behavior in a named
service:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
struct StaminaComponent {
    current: u16,
}

impl EntityComponent for StaminaComponent {}

registry.register(ComponentRegistration::<StaminaComponent>::runtime_only(
    ComponentTypeId::parse("game.stamina")?,
))?;

let revision = state.component_revision::<StaminaComponent>(entity)?;
EntityAuthoringService.attach_component(
    &mut state,
    revision,
    entity,
    StaminaComponent { current: 100 },
)?;
```

The game service validates spends or regeneration, prepares the replacement,
and commits through the same typed boundary. It does not register callbacks or
add another entity-keyed component map. Choose `ComponentRegistration::durable`
with an explicit `ComponentCodec` only when that component belongs in the
entity snapshot.

## Capability-to-component migration

This is an intentional clean public API change; no compatibility aliases are
retained.

| Previous API | Current API |
| --- | --- |
| `TransformCapability`, `BoundsCapability`, `CollisionCapability` | `TransformComponent`, `BoundsComponent`, `CollisionComponent` |
| `RenderableCapability`, `KinematicCapability`, `ControllerCapability`, `AssetBindingCapability` | Corresponding `*Component` types |
| `EntityCapability` / `EntityCapabilityKind` | External `EntityComponent` implementation plus explicit `ComponentRegistration` |
| capability attach/detach operations | `EntityAuthoringService::{attach_component, replace_component, detach_component}` with the exact slot `ComponentRevision` |
| capability activation vocabulary | `ActivatableComponentKind`, `component_activation`, and `set_component_activation` |
| `ContainmentCapability` | `RelationshipCommand::{SetContainment, ClearContainment}` and `RelationshipKind::Containment` |

Rust symbol names changed, but the established schema-3 built-in snapshot field
names did not. Downstream code should reacquire component revisions after
snapshot restoration because those concurrency guards are instance-local.

## Private or forbidden paths

- Do not route every service mutation through `entity-state` command batches.
- Do not add callbacks, subscriptions, renderer handles, I/O, or service
  location to components.
- Do not turn `ComponentRegistry` into a global singleton, service/plugin
  registry, scheduler, or automatic type-discovery mechanism.
- Do not expose unrestricted mutable component references or use Rust type
  names/`TypeId` as durable identity.
- Do not guard an ordinary component write only with the global entity-state
  revision; capture the exact slot revision so unrelated mutations do not
  conflict. Slot revisions are reacquired after snapshot restoration.
- Do not persist closures or arbitrary executable behavior in state-machine
  definitions.

## Acceptance gates and fixtures

```bash
cargo test -p entity-state -p state-machine --locked
cargo clippy -p entity-state -p state-machine --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Focused regressions live under
[`entity-state/tests`](../../rust/crates/entity-state/tests) and
[`state-machine/tests`](../../rust/crates/state-machine/tests).

## Common agent mistakes

- Treating a component as an implicit system callback.
- Adding services, callbacks, or ambient discovery to the bounded data-type
  registry.
- Adding a built-in field or closed enum case instead of registering a new
  downstream component type.
- Confusing an entity snapshot with a universal game save or replay record.
- Moving service-owned state into the entity command batch for convenience.

## Follow-up routing

- Spatial facts and movement: [Spatial mechanisms](spatial-mechanisms.md).
- Durable scenes and entity persistence:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Presentation-only state:
  [Rust render model and projection](rust-render-model-and-projection.md).
