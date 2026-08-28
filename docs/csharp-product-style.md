# C# product style

This is a recommended product coding lane for NativeAOT C# products on Rusty
Engine. It is not an Engine-enforced module framework and it does not claim
that `IProductModule`, `ProductBuilder`, typed projection, typed content, or
analyzer packages already exist. A product can use these practices with
ordinary C# composition today.

## Start with domain ownership

Organize product code around domains a player or designer would recognize,
such as `Combat`, `Equipment`, `Encounters`, `Spells`, or `Player`. Keep the
state, behavior, definitions, and projections for one domain together.

```text
src/
  MyGame.Product/
    MyGameProduct.cs
    Modules/
      Combat/
        CombatState.cs
        AttackInputSystem.cs
        MeleeAttackResolver.cs
        CombatDefinitions.cs
      Equipment/
        EquipmentState.cs
        EquipRequest.cs
        EquipmentView.cs
      Shared/
        Identifiers/
        ValueTypes/
```

This is a layout convention, not runtime module registration. Avoid horizontal
catch-all directories such as `Helpers`, `Managers`, `Utils`, `Processors`, or
`Gameplay` that distribute one domain across the repository.

Each mutable state family has one semantic owner. Give the owner controlled
mutation methods and a clear snapshot/restore representation when the product
needs persistence. Other domains read a shaped view or ask the owner to make a
change; they do not keep mutable references and edit state from the side.

## Keep coordination thin

Use a small coordinator/system at an Engine update boundary when one is needed:

```text
Read → Decide → Apply → Publish
```

- **Read**: collect Engine facts and product-owned views.
- **Decide**: delegate a bounded calculation to a resolver, rule, or policy.
- **Apply**: let the state owner make the admitted product change.
- **Publish**: emit a typed observer-facing fact or presentation/UI input when
  the Engine supports it.

Do not turn every method into a pipeline. A simple local action can remain a
direct method. Likewise, only use a layered rule/resolution structure when the
mechanic genuinely has independently meaningful contributions.

Useful names are descriptive rather than ceremonial:

| Name | Use it for |
| --- | --- |
| `State` | Mutable durable domain state. |
| `View` | Deliberate read-only model of owned state. |
| `Request` | Typed ask for a state owner to perform an operation. |
| `Receipt` | Accepted/rejected result of an operation. |
| `Fact` | Typed record of an accepted gameplay occurrence. |
| `Resolver` | Bounded calculation that produces a decision. |
| `Rule` | A contribution to a genuinely layered resolution. |
| `Policy` | Choice among already valid options. |
| `Projection` | Observer-facing read model; it does not make gameplay decisions. |

Facts and requests are useful contracts, not a reason to build a global event
bus or command framework. Use a direct method when no meaningful ownership
boundary is crossed.

## Use Engine facts; keep product meaning in C#

Use Engine-provided lifecycle/update facts, input, and random services for
gameplay behavior. Do not build a second central loop, poll browser state, use
wall-clock time for gameplay, or introduce unmanaged timers/threads simply to
advance product state.

Keep no retained native pointers or borrowed callback data in product state.
Follow disposable lease/handle ownership from the generated SDK. Ordinary
product projects should contain neither handwritten P/Invoke nor unsafe native
code; generated bootstrap and bindings own that boundary.

Engine owns renderer resources, canvas/backend lifecycle, and game rendering.
C# publishes supported product facts. TypeScript is the DOM UI and explicit
Engine host/backend lane, never the owner of non-UI rendering or gameplay
state.

## C# baseline

- Enable nullable reference types and use file-scoped namespaces.
- Default production types to `internal` and `sealed`; widen visibility only
  for a real cross-domain or package boundary.
- Use records for immutable definitions, snapshots, receipts, and projections.
  Use small `readonly record struct` or value types for IDs and compact facts
  where copying is natural. Use classes for mutable state and coordinating
  behavior.
- Keep one primary production concept per file and let namespaces follow the
  domain directory.
- Prefer explicit composition in the product root. Do not depend on reflection,
  assembly scanning, static-constructor registration, or ambient service
  locators for product behavior.
- Favor `static` lambdas for inline definition behavior when they need no
  captured mutable state. Graduate large behavior into a named resolver, rule,
  policy, or state owner.

These are defaults, not a mandate for warnings-as-errors, a latest analyzer
level, or a particular project-wide analyzer configuration.

## Tuning, identities, and configuration

Never bury a numeric value, timing, range, threshold, content identity, or
other tuning decision in an arbitrary behavior expression.

- At minimum, name a local stable value as a `const` or `static readonly`
  declaration at the owning module so it is findable.
- Prefer typed definitions/options when a value describes a product concept.
- For runtime tuning, use a product-owned typed configuration adapter with a
  clear default and a deliberate load/save policy. This is product code, not a
  requirement for every value.
- Do not create one giant `GameConstants` dump unless a value is genuinely
  shared across domains. Keep domain tuning near the domain that explains it.
- Prefer typed IDs or named definitions when the current API offers them; do
  not scatter raw identity strings through logic.

The goal is to make a value's owner and intent obvious without forcing every
small literal into a global configuration system.

## Boundary reminder

The product decides gameplay semantics and product policy. The Engine
guarantees named infrastructure. If the current generated API lacks a needed
infrastructure capability, record the precise gap and request upstream work;
do not replace it downstream to make a task appear complete.
