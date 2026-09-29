# Same-callback resource release (#8771)

**Question.** Since #8767, the browser fetches resource bodies only from the
runtime's current retained set. What happens when a product creates a mesh or
texture, shows it, removes it and releases it all in one callback?

**Answer.** Meshes: nothing, because their bodies travel inside the frame.
Textures: a full renderer rebaseline on every occurrence. The smallest remedy
is to keep a released body servable through the next call; that is now done.
Same-callback texture use then costs nothing.

## Exercise

`scripts/churn-exercise` is a small SDK product. About once a second it
creates a texture and sprite (or a mesh and mesh appearance) from new bytes,
an identity the browser has never fetched. It publishes an object showing it
with `PublishChanges`, then removes the object and releases the resources:
- **`same`:** in the same update;
- **`next`:** one update later;
- **`hold`:** 30 updates later, as the control.

`scripts/run-churn.mjs` runs it on `rusty-product-host` with Chromium
(SwiftShader) for 20 s. It counts the page's `/runtime/resource` responses and
fresh output streams (every recovery opens one), and samples the page state
every 100 ms.

## Before (`70a7745d`, `results/before-*.json`)

| Kind | Mode | Resource 200 / 404 | Fresh streams after ready | States | Final failure field |
|---|---|---|---|---|---|
| texture | same | 0 / 0 | **20** | ready 187, degraded 2 | `renderer entered terminal state during frame_mutation realization: defineTexture: resource texture-resource/…` |
| texture | next | 20 / 0 | 0 | ready 196 | — |
| texture | hold | 21 / 0 | 0 | ready 195 | — |
| mesh | same | 0 / 0 | 0 | ready 190 | — |
| mesh | next | 0 / 0 | 0 | ready 189 | — |
| mesh | hold | 0 / 0 | 0 | ready 191 | — |

A same-callback texture never reached the browser as a 404. The call's
resource inventory is taken after the call, when the body was already
released, so the browser never asked for it. The frame still defined the
texture, the renderer could not realize a body it had never fetched, and the
page recovered with a full rebaseline. That happened once per churn cycle, 20
in 20 s. Mesh resources never touch the resource route: their payload is
inline in `DefineStaticMesh`.

## Remedy

`RuntimeAppearanceData` keeps each render resource released during a call
(`released_this_call`). At the next call's start it moves them to
`released_last_call`, and the call after that drops them. Both lists are part
of the renderer resource inventory and are served by the resource route
(`composition.rs` `renderer_resource_ids` and `renderer_resource`).

A body released in call N therefore stays fetchable until call N+2 begins, the
same window a release in the next update already had. The cost is holding two
calls' worth of released bodies as `Arc` clones, with no copy.

`a_released_body_stays_servable_through_the_next_call` covers the window.

## After (`results/after-*.json`)

| Kind | Mode | Resource 200 / 404 | Fresh streams after ready | States |
|---|---|---|---|---|
| texture | same (run 1) | 20 / 0 | 0 | ready 195 |
| texture | same (run 2) | 20 / 0 | 0 | ready 194 |
| texture | next | 20 / 0 | 0 | ready 194 |

## Limits

- A fetch that arrives after the call after next still gets 404 and recovers
  with a baseline, as since #8767. #8767's evidence covers that race. The
  window matches the `next` case, which had no misses here.
- The browser fetch path itself goes away when the wgpu backend renders
  in-process (#8786/#8792). This remedy is sized for the current browser
  renderer.
