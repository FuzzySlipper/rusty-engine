# Structured world indicators

Status: implementation contract for task 6856.

## Decision

Structured world indicators extend the existing renderer-neutral billboard
domain. They are not a second gameplay UI model and do not authorize health,
targeting, interaction, or status changes. A downstream game projects the facts
it owns into one stable billboard handle; Engine validates, lays out, realizes,
and disposes that presentation.

Legacy text, value, and icon billboard content remains valid. Structured
content adds a bounded composition of localized label, exact-hash icon,
normalized-at-realization meters, and compact status cues. Callers submit exact
`min`, `max`, `current`, and optional `preview` values. Engine requires
finite values, `min < max`, and values inside that range; a zero or
reversed range is rejected rather than assigned health-specific meaning.

Each meter and status cue has a stable local identity. A content patch may
therefore update value, preview, text, or style in place without replacing the
billboard root or unrelated meter DOM nodes. Composition is capped at four
meters and eight status cues. Text, texture, color, segment, dimension, and
aggregate limits reject before projector publication.

## Sizing and layout

Structured indicators declare neutral pixel dimensions and one optional layout
policy. Two sizing policies are supported:

- constant pixels; and
- bounded distance scaling with an explicit reference distance and minimum and
  maximum scale.

World-sized DOM is deliberately unsupported. It couples CSS realization to a
camera/world unit policy and is not needed by the current consumer. CSS pixel
geometry remains stable under device-pixel-ratio changes; the browser owns the
physical-pixel rasterization.

The host projects every valid anchor, then sorts candidates by descending
caller priority and ascending stable handle. It applies per-edge safe areas,
explicit clamp-or-cull edge behavior, a fixed submitted/visible quota, and the
selected stack-or-suppress overlap behavior. Equal facts therefore retain an
equal ordering across frames. Unchanged placements are retained within a small
screen-space hysteresis envelope so subpixel camera movement does not reshuffle
overlapping indicators.

Crowd suppression is presentation-only readout. It never changes the caller's
descriptor or gameplay visibility decision.

## Depth and accessibility

The existing layers retain their meaning:

- `alwaysOnTop` ignores scene depth;
- `depthTested` follows projected depth ordering but has no GPU depth sample;
  and
- `occluded` hides only when the configured host projector supplies an
  occlusion observation.

The public application host currently supplies CPU camera projection and may
report occlusion as unavailable. The private fixed host can supply stronger
host observations when it has them. Neither path invents a GPU readback or
feeds visibility into gameplay authority.

Browser realization is pointer-transparent. The root receives a useful
accessible name; meters use progress semantics with exact minimum, maximum,
current, and localized names; decorative pieces are hidden from the
accessibility tree. These DOM choices are backend realization, not fields that
expose elements, selectors, or templates to Rust.

## Lifecycle and ownership

```text
game-owned facts
      |
      v
bounded descriptor/patch + exact assets + stable handle
      |
      v
Rust projector stages complete presentation frame
      |
 failure --> prior projector state remains live
      |
      v
strict TypeScript decode + resource preparation
      |
      v
stable DOM subtree + deterministic screen layout
      |
      v
host readout: visible / culled / suppressed / diagnostics
```

Engine owns descriptor validation, projection lifecycle, stable layout,
resource hash verification, host realization, diagnostics, and disposal.
Downstream owns source facts, semantic priority, requested visibility, camera
orchestration, localization inputs, and product styling choices.

Missing entity anchors, unavailable resources, host absence, invalid or stale
frames, resize, reset, and disposal are explicit. Renderer observations remain
one-way. No callback, arbitrary DOM, selector, HTML template, input binding,
URL fetch, or game scheduler enters the contract.

## Public integration

Rust callers use the `rusty_engine::render_presentation` namespace. Browser and
wrapper products submit the resulting strict presentation JSON through
`@rusty-engine/application-host`; its Engine-owned overlay and billboard host
perform projection and DOM realization. Downstream code does not import Three
or construct indicator DOM.

The fixed `renderer-webview-host` uses the same descriptor and private host
implementation. A future non-DOM backend can realize the same neutral
composition without changing game facts.

## Performance evidence

The real-Chromium renderer-host characterization covers an idle refresh and a
changing-meter refresh with 100 visible indicators, plus bounded degradation
from 500 submitted indicators. Run it with:

```bash
./scripts/measure-world-indicators.sh
```

On the task 6856 development host, its 21-sample median was 0.6 ms for idle
layout refresh and 4.8 ms for a value-only update of all 100 indicators. The
single 400-descriptor addition taking the host from 100 to 500 active
descriptors took 20.7 ms; deterministic suppression left exactly 256 visible.
These are characterization numbers, not a hardware-independent gameplay
budget.

The durable budget is structural: at most 500 submitted descriptors per host,
256 visible roots by default, four meters and eight status cues per structured
descriptor, deterministic suppression beyond the visible/layout budget, and no
subtree rebuild for a value-only patch with stable local identities.

## Known limitations

- GPU depth-buffer occlusion is not read back by the ordinary DOM host.
- Text measurement uses declared indicator dimensions rather than exposing DOM
  layout as authority.
- Interactive diegetic panels, render-to-texture controls, pointer routing, and
  focus policy are intentionally unsupported. They require a concrete consumer
  contract separate from ordinary pointer-transparent indicators.
- Arbitrary rich text, HTML, CSS selectors, callbacks, and product templates are
  not accepted.

No follow-up for interactive panels is created without a concrete consumer.

## Verification

```bash
cargo test -p render-presentation --locked
cargo clippy -p render-presentation --all-targets --locked -- -D warnings
pnpm --dir render --filter @rusty-engine/render-contracts test
pnpm --dir render --filter @rusty-engine/renderer-host test
pnpm --dir render --filter @rusty-engine/application-host test
./scripts/verify-render.sh
./scripts/verify-application-host-artifact.sh
./scripts/verify-renderer-webview-host.sh
```
