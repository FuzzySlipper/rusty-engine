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
- [`entity-state/src/character_motion.rs`](../../rust/crates/entity-state/src/character_motion.rs)
- [`entity-state/src/character_motion_publication.rs`](../../rust/crates/entity-state/src/character_motion_publication.rs)
- [`entity-state/src/rigid_body.rs`](../../rust/crates/entity-state/src/rigid_body.rs)
- [`entity-state/src/rigid_body_publication.rs`](../../rust/crates/entity-state/src/rigid_body_publication.rs)
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
- `EntityAuthoringService::replace_components<T>` is the bounded exception for
  one service that must publish several slots of the same component type. It
  admits at most 32 unique entity slots, validates every exact revision and
  candidate before writing, and advances the global revision once.
- Containment keeps the forward ownership relationship plus a maintained
  identity-ordered direct-child index. `contained_entities` and
  `contained_entity_count` are owner-local reads, not population scans.
- Downstream Rust composes these mechanisms directly with named game services.
- Serialized entity snapshots are durable facts only where the owning consumer
  chooses to persist them.
- `RenderableComponent::local_transform` is presentation-only. Entity/world
  transform remains authoritative for every spatial and gameplay consumer;
  render projection alone composes the visual-local value.
- Runtime-only registrations are omitted from snapshots. Durable downstream
  components require an explicit codec identity/version and the same registry
  during decode; built-in schema-3 fields retain their established JSON shape.
  Identity visual-local transforms remain omitted so earlier snapshots decode
  to the same presentation.
- `RigidBodyComponent` is the built-in durable registered exception: its stable
  schema-1 codec stores inert non-kinematic body facts, while the named spatial
  service owns all solver behavior and exact-slot publication. No backend
  handles or callbacks are components.
- `replace_rigid_body_states` is a second deliberately narrow publication
  exception: it atomically replaces the exact transform and rigid-body slots
  prepared by the spatial service. It is bounded to 1,024 unique entities and
  validates every candidate and slot guard before writing.
- `CharacterMotionComponent` is the built-in durable schema-1 inert state for
  the named spatial character controller. It stores controlled/external
  velocity, stance/grounding and jump timers, stable support continuation,
  neutral fall-height facts, command sequence, and collision-world identity;
  contacts and solver caches remain derived service readouts.
- `replace_character_motion_state` is the narrow single-character publication
  seam. It validates the complete candidate, requires exact Transform and
  character-motion slot revisions, rejects legacy-kinematic/rigid-body,
  parenting, and non-unit-scale conflicts, and atomically changes both slots
  with one global revision advance or changes neither.

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
- Do not broaden homogeneous component replacement into a heterogeneous
  transaction, command AST, callback pipeline, or generic service-owned state
  route.
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
cargo test -p entity-state character_motion --locked
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
