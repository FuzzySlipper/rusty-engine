# C# product style

This is the recommended coding style for C# products on Rusty Engine. It is
a set of conventions, not an Engine module framework: the SDK has no
`IProductModule`, `ProductBuilder`, typed projection, typed content, or
analyzer packages. A product applies these practices with ordinary C#
composition.

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

Each mutable state family has one semantic owner. Ordinary classes, direct
references and domain methods are the baseline; an attached EntityStore component
is the same object the product constructed. Use domain methods where they preserve
useful invariants. Add a shaped view or request when an ownership boundary benefits
from it, and an explicit capture/rebuild representation when persistence is needed.
Sharing references is deliberate C# aliasing, not an Engine permission boundary.
For the Engine mechanics vocabulary (`EntityStore`, `EntityTypeId`, `Actor`,
`Stat`/`Track`, `StatsComponent`/`EffectsComponent`, inventory facades, explicit
save/capture/debug), see [managed helpers](csharp-helpers.md) and the
[capability map](csharp-capabilities.md#managed-helpers-in-the-default-assembly).

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
| `State` | Mutable domain state; persistence is optional. |
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
Follow disposable handle ownership from the generated SDK. Ordinary
product projects contain neither handwritten P/Invoke nor unsafe native code;
the generated bind export and the SDK own that boundary.

Engine owns renderer resources, canvas/backend lifecycle, and game rendering.
C# publishes supported product facts. TypeScript is the DOM UI and explicit
Engine host/backend lane, never the owner of non-UI rendering or gameplay
state.

Keep implementation topology in developer documentation. Do not add static
"runtime connected" or "product host active" banners to product UI: they do
not establish that gameplay is running. Show actionable failures and requested
diagnostics through the Engine's diagnostics and UI surfaces.

## Game UI and diagnostic UI

Rusty Engine games ship as desktop apps. Players see the product UI page in
the desktop window, where an off-screen web engine (Servo, or Chromium)
composites it over the scene.
During development the same page also runs in a browser tab, which is where
agents and tools drive it most easily. The page is ordinary DOM either way,
and its UI is one of two kinds:

- **Game UI** is what a player uses: HUD, inventories, menus, dialogue,
  settings. It keeps to the green path below, which behaves the same in a
  browser, the stream and the desktop window. Iterate in the browser, but
  check game UI in `rusty dev --output window` before calling it done,
  because the window is what ships.
- **Diagnostic UI** is for developers and agents: debug panels, tuning,
  inspection, test controls. It may use any browser feature, and exercising
  it only in a browser is fine. In the window an unsupported feature does
  nothing, or warns in the page's console, and never locks input
  ([browser features in the window](desktop-shell.md#browser-features-in-the-window)).

Keep the two apart in the page, so that a debug panel's needs never shape a
game surface.

The green path:

- **Input.** Gameplay input reaches the product through the Engine's input
  capture. UI controls take DOM pointer events (`click`, `pointerdown`,
  `pointermove`, `pointerup`) and keyboard events.
- **Escape and focus.** Escape ends a pointer lock or cursor confinement
  first; the page sees the lock end, not the key. Drop held state and cancel
  drags on the window's `blur`.
- **Menus and modals.** Draw them in the DOM; `<dialog>` with `showModal()`
  works everywhere. Never use `alert`, `confirm` or `prompt`: the window
  cancels them. For a right-click menu, call `preventDefault()` on
  `contextmenu` and draw the menu; the window shows no native one.
- **Lists and choices.** Overflow lists scroll with the wheel. `<select>`
  works, single or `multiple`, and so do `date`, `time` and
  `datetime-local` inputs; the window draws their lists and pickers. Keep
  to those types: `month` and `week` inputs are text fields under Servo
  ([UI engine](desktop-shell.md#ui-engine)).
- **Styling.** Servo 0.7 lacks `:has()`, `::backdrop` and `autofocus`, and
  sizes form controls its own way: style game UI without them, and size
  controls explicitly.
- **Text entry.** Use `<input>` and `<textarea>`. IME composition has not been
  checked in the window.
- **Tooltips.** Draw them on `pointerenter`/`pointerleave`; the window never
  shows `title` tooltips.
- **Inline SVG.** Under Servo an `<svg>` is drawn from its markup alone
  ([inline SVG](desktop-shell.md#inline-svg-in-the-window)). Give shapes
  presentation attributes (`fill`, `stroke`, `display`) and change those
  attributes from script for selection, focus and visibility; changing them
  in place redraws. Give `<text>` a `font-family` attribute. Put anything
  the player points at or clicks in HTML elements placed over the `<svg>`:
  Servo sends pointer events to the `<svg>` element, not to the shapes
  inside it.
- **Pickers.** Draw file and colour choices in the page. No native file or
  colour picker opens in the window.
- **One page.** Game UI stays in the one page. Navigating replaces the UI, and
  `window.open` and `target="_blank"` open nothing.
- **Drag and drop.** Use pointer events, as described next.

### Drag and drop

Drag with pointer events, not HTML5 drag and drop. The desktop window refuses a
native drag: it ends at once with `dropEffect` `none`, and the page's console
says so.

- Keep native drag out of the surface. Images and links are draggable by
  default, and a native drag ends the pointer sequence with `pointercancel`,
  so set `draggable="false"` on them (or call `preventDefault()` on
  `dragstart`).
- On `pointerdown` over an item, call `setPointerCapture(event.pointerId)` and
  remember the item. Start the drag after a few pixels of movement, so a
  click stays a click.
- On `pointermove`, move a ghost element styled `pointer-events: none` and
  find the slot under the pointer with `document.elementFromPoint`.
- On `pointerup`, find the slot the same way and ask the product to move the
  item. C# owns the inventory: the UI sends the request and redraws from what
  the product publishes.
- Cancel on `pointercancel`, `lostpointercapture`, Escape and the window's
  `blur`.

```ts
item.draggable = false;
item.addEventListener('pointerdown', (down) => {
  item.setPointerCapture(down.pointerId);
  let dragging = false;
  const slotAt = (e: PointerEvent) =>
    document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-slot]');
  const move = (e: PointerEvent) => {
    dragging ||= Math.hypot(e.clientX - down.clientX, e.clientY - down.clientY) > DRAG_START_PIXELS;
    if (dragging) ghost.show(item, e.clientX, e.clientY, slotAt(e));
  };
  const end = (e: Event) => {
    item.removeEventListener('pointermove', move);
    item.removeEventListener('pointerup', end);
    item.removeEventListener('pointercancel', end);
    ghost.hide();
    const slot = e.type === 'pointerup' && dragging ? slotAt(e as PointerEvent) : null;
    if (slot) requestMove(item.dataset.slot!, slot.dataset.slot!);
  };
  item.addEventListener('pointermove', move);
  item.addEventListener('pointerup', end);
  item.addEventListener('pointercancel', end);
});
```

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
