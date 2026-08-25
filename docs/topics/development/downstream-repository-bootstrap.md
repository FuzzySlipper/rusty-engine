# Downstream repository bootstrap

This is the practical starting point for an outside agent or developer creating
a new Rusty Engine product. It explains how to arrange the provider and product
checkouts before choosing gameplay vocabulary, content, or a packaged host.

For the architecture and code philosophy behind the layout, continue with the
[greenfield downstream product path](greenfield-downstream-product.md). For a
working minimum, use [Rusty Template](https://github.com/FuzzySlipper/rusty-template).
The template is a starting point, not a framework, dependency updater, or
universal game grammar.

## Create one shared root

Keep the Engine and product repositories adjacent:

```text
rusty-workspace/
  rusty-engine/
  rusty-template/       # rename or generate from this for a new product
```

One straightforward setup is:

```bash
mkdir rusty-workspace
cd rusty-workspace
git clone https://github.com/FuzzySlipper/rusty-engine.git
git clone https://github.com/FuzzySlipper/rusty-template.git
cd rusty-template
./scripts/bootstrap.sh
./scripts/verify.sh
```

The operator creates this layout. Downstream source, build scripts, and ordinary
CI must not fetch, pull, reset, clean, checkout, or pin `../rusty-engine`.
They consume the checkout they were explicitly given. A clean CI runner needs
an external integration harness or workspace setup step that supplies both
repositories before downstream verification begins; that checkout policy does
not belong inside the product.

## What adjacency provides

The ordinary Rust dependency is the complete facade:

```toml
[dependencies]
rusty-engine = { path = "../../rusty-engine/rust/crates/rusty-engine" }
```

The exact number of `..` segments depends on the consuming crate's location.
There should still be one facade dependency in the downstream workspace, not a
hand-picked collection of private Engine crates.

A rich-DOM browser or WebView shell consumes only the published local
application-host artifact:

```json
{
  "dependencies": {
    "@rusty-engine/application-host":
      "file:../../rusty-engine/render/artifacts/application-host"
  }
}
```

The template bootstrap checks that this Engine-owned artifact exists but never
writes to the sibling checkout. If the artifact is absent, build it explicitly
from `../rusty-engine/render`, then rerun the downstream bootstrap. Product
TypeScript must not import renderer internals, Three, or the private webview
bridge.

## What the template proves

Rusty Template deliberately implements one small vertical path:

```text
product TypeScript authoring
  -> checked product artifact
  -> product admission and runtime owner
  -> renderer-neutral cube frame
  -> public application-host
  -> one viewport, one Engine canvas, one bounded product UI label
```

The TypeScript authoring layer is build-time syntax. The product semantic layer
defines and validates the serialized meaning. The initial browser frame is
exported as a deterministic smoke artifact; TypeScript presents it but does
not evaluate or mutate gameplay. A real product can later replace that export
edge with one named product service without changing the renderer or UI authority
boundary.

The cube proves compilation, product-content admission, Rust projection, the
public renderer path, real WebGL output, and bounded viewport composition. It
does not prove a live gameplay transport, save system, scheduler, or packaged
desktop lifecycle.

## Repository owners

The minimum template keeps these responsibilities visible:

```text
crates/                 product gameplay, state, admission, projection
gameplay/authoring/     optional pure TypeScript builders and materialization
content/gameplay/       checked artifacts admitted again by the product runtime
apps/web/               thin application-host composition and local UI
tests/                  product and browser-owned evidence
scripts/                verification and deterministic build plumbing
```

Start new live semantics in the product runtime. Add TypeScript authoring only
after the owning semantic layer defines the wire shape and compiler. Keep HUD,
menu, and accessibility state inside the
UI root supplied by `mountRustyApplication`; never use the document, hidden
offscreen controls, or browser storage as gameplay state.

## Product viewport posture

A game-style product is not an infinitely growing web page. Its browser or
WebView composition root should:

- mount `mountRustyApplication` once;
- pass finite `presentationAspectBounds`;
- retain the sole Engine-owned canvas;
- mount all ordinary product UI inside the supplied bounded UI root;
- keep `html`, `body`, and the application root constrained to the viewport;
- allow scrolling only inside an intentionally bounded panel;
- call `context.ui.allowsGameplayInput(event)` before a global DOM listener
  assigns gameplay meaning; and
- avoid a second render loop, copied canvas geometry, offscreen controls, or
  document-level overflow.

Read the complete [downstream renderer and Studio boundary](downstream-renderer-and-studio.md)
before adding a host or renderer integration.

## Guidance for agents without Den

The committed Engine repository is intentionally understandable without Den.
An outside agent should read:

1. the downstream product's own `AGENTS.md`;
2. this bootstrap document;
3. the [greenfield product guide](greenfield-downstream-product.md);
4. the [canonical Engine design](../../design.md); and
5. the [agent code atlas](../../agent-code-atlas.md) when changing Engine.

Den may own current task and review state when a task is supplied. It is not a
prerequisite for understanding the checked-out provider or template.

Cross-repository relative Markdown links do not work on GitHub. A downstream
`AGENTS.md` may name a local path such as
`../rusty-engine/docs/topics/development/greenfield-downstream-product.md` for
agents operating in the shared checkout, but it should also provide the
[canonical GitHub link](https://github.com/FuzzySlipper/rusty-engine/blob/main/docs/topics/development/greenfield-downstream-product.md)
for remote readers.

## Verification layers

Keep the claims separate:

- Rust tests prove admission, product state, and renderer-neutral projection.
- Authoring checks prove deterministic TypeScript materialization and drift.
- TypeScript typecheck/build proves the composition root and public artifact
  usage.
- Real browser evidence proves one canvas, actual rendered pixels, bounded
  geometry, no document overflow, and cleanup at the browser-owned layer.
- A future Tauri or other packaged host needs its own headed lifecycle proof;
  browser success does not certify it.

Run the template's `./scripts/verify.sh` for its complete current proof. Engine
provider changes still use the owning Engine gates; a downstream green result
does not replace them.
