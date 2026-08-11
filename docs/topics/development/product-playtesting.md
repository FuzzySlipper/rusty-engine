# Product playtesting and evidence authority

Black-box product playtesting observes a running product through its public
output and ordinary input. It is useful evidence, but it does not become an
Engine mechanism, a deterministic test oracle, or a second gameplay authority.

## Three evidence layers

Use the smallest layer that owns the claim:

1. **Rust mechanism evidence** exercises host-neutral state, services,
   projection, mutation, persistence, and failure atomicity. It belongs in the
   focused crate test and ordinary provider gate.
2. **Renderer and application-host evidence** exercises strict frame decoding,
   retained resources, Three/WebGL realization, DOM/input arbitration,
   replacement, and lifecycle cleanup. It belongs in the isolated renderer or
   host gate.
3. **Final-product playtesting** operates visible output and public keyboard,
   mouse, controller, or native-host input. It judges whether the composed
   product behaves coherently for a user. It belongs in completion, review,
   nightly, or release evidence rather than every-commit CI.

One layer does not silently substitute for another. A model observing a moving
camera does not prove a Rust invariant. A green projection unit test does not
prove pointer capture or the visible product. Browser and native adapters are
separate evidence lanes even when they share the same scenario intent.

When exploratory playtesting confirms a behavioral defect, add the smallest
deterministic regression at the owning mechanism or host boundary when
practical. Do not encode the entire exploratory session as a brittle scripted
test.

## Authority boundary

Ordinary Engine Rust remains host neutral. It does not depend on Playwright,
Chromium, models, Node, Studio, public URLs, or sibling game checkouts merely
because a product can be playtested in a browser.

Downstream Rust continues to own gameplay facts, meaning, orchestration, and
semantic input mapping. It reaches presentation through the complete Rust
facade and the public host contracts. A browser/Tauri/Electron product may use
the single `@rusty-engine/application-host` artifact. Playtesting is not a
reason to add a second canvas, renderer graph, bridge, control route, internal
renderer handle, or hidden authoritative readout.

The local Den playtest broker owns browser/session tooling and evidence
packets. Its permissive diagnostics may inspect the host when debugging, but
those diagnostics are labelled evidence and never become a production API or
the basis for product meaning. Visible judgement should use repeated
screenshots or frame bursts around ordinary input; diagnostics may corroborate
but must not replace that judgement.

## Engine public-host fixture

The bounded fixture is rooted at:

- [`render/product-playtest`](../../../render/product-playtest);
- [`.den-playwright.json`](../../../.den-playwright.json); and
- [`render/product-playtest/scenario.json`](../../../render/product-playtest/scenario.json).

Its composition root imports only `@rusty-engine/application-host`. The
fixture supplies one renderer-neutral product frame and ordinary downstream
WASD/mouse semantics through the public renderer and interaction ports. It
does not expose a global host object, test hook, canvas handle, WebGL/Three
object, alternate control channel, application readback, or sibling checkout.

The fixture proves that the generic playtest substrate can operate an
Engine-hosted product. It does not certify a downstream game. The manifest and
live model session remain outside `./scripts/verify.sh`; deterministic fixture
typechecking and boundary checks belong to `./scripts/verify-render.sh`.

Run the on-demand fixture with the task-6784 local CLI/MCP installation. For a
Codex worker, pass the repository, exact revision, scenario contents, and
artifact preferences from the files above. The expected lifecycle uses only
the eight `playtest_*` tools and finishes with an indexed evidence packet.

## Minimal downstream adoption

A downstream product needs one root manifest with its existing product start
command and public route:

```json
{
  "project": "my-product",
  "serve": {
    "command": "my-product-serve --host {host} --port {port}",
    "preferredPort": 0,
    "healthUrl": "/healthz",
    "readyText": "my-product",
    "reusePolicy": "broker_owned"
  },
  "playtest": {
    "startPath": "/game",
    "viewport": { "width": 1280, "height": 720 },
    "recordVideo": false
  }
}
```

Keep scenario intent beside product-owned operational guidance:

```json
{
  "scenario": "walk-to-exit",
  "mission": "Enter gameplay, walk to the visible exit, and look back at the start.",
  "controls": ["Click to capture", "WASD to move", "Mouse to look"],
  "viewport": { "width": 1280, "height": 720 },
  "artifacts": {
    "screenshots": true,
    "frameBurst": { "count": 6, "intervalMs": 100 },
    "trace": true,
    "video": false
  }
}
```

The parent agent supplies that scenario to the playtester; it is not a new
Engine schema or gameplay language. The product does not copy renderer code or
know the TypeScript renderer package topology.

Record live evidence separately with exact SHA, clean/dirty state, scenario,
outcome, discrepancies, timeline/artifact offsets, model judgement versus
diagnostics, absolute index path, and cleanup result.
