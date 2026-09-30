# C# managed helpers

Optional managed helpers, entity and inventory stores, ordinary numeric stats and resource tracks. Entry page: [C# SDK guide](csharp-sdk.md).

## Optional managed helpers

These namespaces are reusable helpers compiled into `Rusty.Engine`, not
required product framework pieces:

| Namespace | Role |
| --- | --- |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | An optional Engine-context update pipeline and deterministic scheduler helper. Its `SimulationScheduler` can resume on the next admitted step, wait fixed admitted steps, or wait for a caller-owned completion condition without creating a second clock. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | Ordinary class/value component storage, `EntityBatch` grouped writes, and managed adapters around Engine mechanisms. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Explicit product-state codecs and stores for the current shape. There are no product schema versions or migrations: changing the shape breaks old saves, by product choice. |

Use a helper when it fits the product's real domain. A product may compose its
own ordinary C# architecture instead. The SDK has no `ProductApplication`,
`ProductBuilder`, `IProductModule`, analyzer suite, typed-content framework,
or projection framework.

Product- and Kit-owned typed services with meaningful rule-resolution extension points
are ordinary C# composition, not Engine framework extension: the Engine ships no gameplay
bus, plugin registry, or RPG rules, and Read → Decide → Apply → Publish is a
product-style option (see [C# product style](csharp-product-style.md)), never an Engine
protocol.

### Entity stores, mechanics stores and Engine adapters

`EntityStore` holds managed entity/component facts; `InventoryStore` holds the
inventory, item and equipment ledger. Their identities and revisions are local
to their owning stores. The entity adapters read those facts and call named
Engine mechanisms; they do not create another entity world or own native
resources supplied by the caller.

Adapter receipts follow their owning adapter name.
`InventoryView.StoreRevision` identifies the whole inventory
store revision; its `InventoryRevision` identifies the individual owner
inventory revision. Item receipts use `InventoryRevisionBefore` and
`InventoryRevisionAfter`. Debug registration uses
`RegisterStore`, `ReplaceStore` and `UnregisterStore`; `entity.stores` lists
registrations and debug output identifies them with `store=` / `stores=`.

`DynamicsWorld` is a disposable native simulation owner. `WorldOrigin` is the
spatial coordinate origin. Neither is a managed entity
store. `EntityOriginRebaser.Prepare` computes each root's local transform in
the target frame from its global position; `Commit` moves the origin and
rebases the live collision scene. Voxel or collision edits made between the
two are kept, and several prepared rebases may commit in any order: the last
commit decides the origin. `Commit` also rebases every stored `CharacterMotion`,
whose support anchor, fixed tether anchor and fall/peak heights are local-frame
values. A product that holds character motion itself applies
`motion.Rebased(receipt.LocalDelta)` after the commit. Without it, the next step
on a support carries the character back by the whole origin delta, a fixed
tether re-attaches at full length, and a landing measures its fall from the old
peak. Local-frame values the product passes in each step (a fixed tether
anchor, support and obstacle transforms) move by the same `LocalDelta`.

### Ordinary component attachment

```csharp
using var entities = new EntityStore();
EntityId actor = entities.Create();
var health = new Health { Current = 10 };
entities.Add(actor, health);
entities.Get<Health>(actor).Current -= 2;
// health.Current is now 8: this is the same attached object.
foreach (var row in entities.Query<Health>())
    Console.WriteLine($"{row.Entity.Value}: {row.Value.Current}");

sealed class Health
{
    public int Current { get; set; }
}
```

No component base class, interface, numeric key, registration or codec is required.
The explicit generic `T` selects one family per entity: `Add` rejects an occupied
slot, `Replace` requires an existing slot, and `Remove<T>` returns false when absent.
Null components are rejected. `Has<T>` and `TryGet<T>` inspect membership without
registering a family. An interface/base family is available by explicitly choosing
that generic type; the store does not scan an object's inheritance hierarchy.

`Get` and query rows return the actual class instance. The normal C# aliasing rules
apply: components usually have one semantic owner, but deliberate sharing is
allowed. Removal, replacement, entity destruction and store disposal release
attachments without invalidating references already held by product code and
without disposing the component or its native resources. Product owners perform
any necessary cleanup. Store IDs are local to one store lifetime.

Queries capture membership immediately, in increasing entity-ID order. Adding,
removing, replacing or destroying attachments afterward does not change that
returned list. Class objects within the list remain live references, including
objects subsequently detached from the store. Disabled entities are excluded
unless `includeDisabled: true`; two-family `Query<TFirst, TSecond>` joins use the
same rules. Use the store at the product's normal execution boundary, not as a
concurrent object database.

Useful value facts are structs. `Set(entity, value)` explicitly inserts or
replaces a struct; `Add`/`Replace` also work for them. Value reads are ordinary C#
copies, so nested references are shared unless the product explicitly copies
them. Prefer a class for mutable reference-bearing state. Registered
`ComponentType<T>` descriptors and generic access address the same family, not
parallel storage. A descriptor can be registered after generic attachment; a
second explicit descriptor for that same `T` is rejected. Descriptor keys used
by diagnostics for automatically attached families are store-local implementation
details, not durable serialization identities.

Versions describe explicit attachment, removal, replacement and lifecycle
changes. They do not observe fields or methods on a returned object, and replacing
a class with the same instance is a no-op. Destroy releases entity/component rows
and containment edges; it detaches children without destroying them. IDs stay
nonzero and monotonic, with no reuse.

### Entity metadata and the optional Actor facade

`EntityStore.Create` accepts an `EntityTypeId`: kind/origin metadata describing what an
entity is, independent of its unique runtime `EntityId` and any product-owned durable
identity. The value is free-form and fixed at creation — `"code:spawn/goblin-scout"` needs
no authored definition or registry — and defaults to `EntityTypeId.Unspecified`. Metadata
travels with the canonical record and shows in `entity.list` / `entity.get` debug output.
Read it with `GetTypeId`; it is not a component.

`Actor` is an optional sealed facade over one existing entity for discoverable typed access:

```csharp
var actor = new Actor(entities, hero);
StatsComponent stats = actor.Get<StatsComponent>();
EntityTypeId kind = actor.TypeId;
```

Every member reads the store live, so named properties return the same attached instances —
never copies or a mirrored state graph. Wrapping attaches nothing: unknown entities throw,
missing components throw the store's ordinary `InvalidOperationException`, and releasing the
facade never affects the entity (there is nothing to dispose). Downstream Kit and ruleset
actors compose their own small wrappers holding an `Actor` rather than inheriting from it.

### Explicit edits and persistence

Ordinary access never deep-copies components. Direct gameplay methods need no
transaction session or receipt graph.

Every `EntityStore` write applies directly. `EntityStore.Revision` and the entity and component revisions are change
counters you may read to skip work. `EntityBatch.Set` and `EntityBatch.Create`
list several writes, and `EntityStore.Commit(batch)` applies them in order and
returns the store revision before and after. A failing write leaves the earlier
ones applied, like any sequence of writes; nothing is rolled back.

D20 uses batches for value facts. The entity adapters (character, Dynamics,
kinematic, motion, trigger, world-origin and graphics) read the store, make their
native call, and write the results directly.

`InventoryEdit` groups inventory operations that must apply together, such as a
payment and a grant. Its operations run on a working copy and apply on `Publish`;
a failed operation, cancellation or disposal leaves the store unchanged. Because
the working copy replaces the store's contents, `Publish` refuses if the store
changed directly after the edit began, so that change is never lost.

Explicit saves use `ProductStateStore<T>` with a product-defined codec and data.
The product decides what to capture, validates/rebuilds a candidate when needed,
and adopts it. Bounded debug snapshots are observations, not live state owners.

## Ordinary numeric stats

`Rusty.Engine.Mechanics.Stat` is one mutable, double-backed object for whole-number
and fractional gameplay values. It works independently or inside a class component.

```csharp
var strength = new Stat(40, minimum: 0, maximum: 100);
var equipment = strength.AddModifier(4);
strength.AddModifier(1.5, StatModifierKind.Multiply);
int attackStrength = strength.ValueInt; // (40 + 4) * 1.5 = 66
strength.RemoveModifier(equipment);
strength.BaseValue = 42;

var speed = new Stat(1, minimum: 0, maximum: 1);
var slow = speed.AddModifier(0.4, StatModifierKind.Maximum);
float movementFactor = speed.ValueFloat;
speed.RemoveModifier(slow);
```

`Value` is a double. `ValueFloat`, `ValueInt` and `ValueInt64` are explicit
conversions; out-of-range conversions throw instead of wrapping or producing
infinity. Integer getters default to nearest with midpoint away from zero.
Set `integerRounding: MidpointRounding.ToZero` for rules that truncate; this does
not round the underlying value. There are no implicit numeric casts.

Base values and contributions must be finite. Default bounds cover finite double
values, with no fixed gameplay ceiling. `Minimum`/`Maximum` or `SetBounds` change
bounds. `quantum: 0.25` rounds evaluations to quarter units; zero (the default)
disables this. `rounding` selects the evaluation rounding mode separately from
integer conversion. Selected additions precede multipliers, then quantization
and the final clamp to resolved bounds. Off-grid endpoints win: a rounded 0.5
with maximum 0.4 produces 0.4. Invalid arithmetic, bounds or modifier changes
leave the previous valid stat and modifiers unchanged.

For authored provenance, `SetSources(statId, sources)` accepts `StatSource` values
with typed contributions, priorities and stacking groups. Sources sort by priority,
identity and definition; equal-strength selections keep the first in that order.
`UniqueByDefinition` selects the first activation of each source definition.
`RemoveSource` removes one activation; `Explain()` returns an immutable evaluation
readout with applied, suppressed and inapplicable decisions. These are optional;
ordinary `AddModifier` needs no source identities or operation records. Local
modifiers keep insertion order and precede authored contributions within each
operation phase. Modifier handles are local to their creating stat.

Inventory quantities, capacity and entity identities are checked integers and
do not pass through floating-point stats.

A refused inventory, equipment, effect, stat or track operation throws
`MechanicsException`. Branch on its `Reason` (`MechanicsRefusal`, such as
`StackMaximum` or `Capacity`), not on its message.

## Resource tracks

A `Track` references its actual maximum `Stat` and owns its current value:

```csharp
var maximum = new Stat(100, minimum: 0);
var health = new Track(maximum, current: 70);
health.Spend(10);
maximum.BaseValue = 120; // health is now 60/120
health.Restore(200);    // clamps to 120
bool paid = health.TrySpend(130); // false, current stays 120
```

The fixed-maximum convenience `new Track(100)` creates a Stat internally. Current
defaults to the maximum; minimum defaults to zero. `Maximum` returns the same Stat
object, `MaximumValue` reads its value, and `Current`/`Value` read the track value.
`ValueFloat`, `ValueInt` and `ValueInt64` provide the same checked conversions as
Stat. Spending/restoring rejects negative or nonfinite amounts. `Spend` throws on
insufficient value; `TrySpend` returns false without mutation. `Restore` saturates.
Spend/Restore return the actual applied amount. `SetCurrent` rejects out-of-bounds
values unless explicitly called with `clamp: true`.

By default, maximum changes preserve current and clamp it to the new bounds.
`maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount` instead
preserves the missing amount: 70/100 becomes 90/120; 90/100 becomes 70/80.
All tracks sharing that Stat reconcile synchronously before a Stat mutation
returns. If a dependent track would have maximum below minimum, the entire Stat
change is rejected before any dependent changes. Dependencies are weak references;
abandoned tracks do not keep imposing their minimum or require disposal.

Track quantization/rounding and integer conversion are constructor policies,
independent of its maximum's policies. Use `quantum: 1` and explicit rounding for
whole-number rules. Endpoints remain reachable even off the grid. Changing Minimum
validates first and clamps current as needed. Direct operations apply
immediately.

For an actual preview, `Stat.Copy()` creates independent numeric state without
copying dependent tracks. A product can construct a new Track around that copy,
validate its grouped plan and adopt its chosen state. D20 uses this for action
planning and Dagger for level-up preflight; this is explicit product orchestration,
not a promise of automatic graph rollback. Copies receive independent local
modifier handles; authored source identities remain available for source removal.
