# Rusty Engine Studio

This is the isolated first-party Studio workspace. It targets full useful feature parity with
the pinned Asha Studio donor while consuming Rusty Engine's canonical Rust owners and shared
renderer.

M11A contains the source inventory, dispositions, owner-adoption map, dependency boundary, and
workspace gate. M11B supplies the structural TypeScript client for the real project-owned Loading
Bay Rust adapter. M11C supplies the Angular/Nx editor shell. M11D makes canonical authored-scene
hierarchy, typed transform settlement, and shared-renderer lifecycle/grid/picking operational.
M11E adds project-embedded voxel assets and transformed instances, material/palette authoring,
Rust-revalidated voxel picking, atomic brush edits, durable history, typed annotations, bounded
model queries, and private-plan GLB conversion through the same renderer. Protocol 4 completes the
M11F voxel reconciliation with trusted host asset/GLB files, deterministic templates and
environments, every primitive and annotation edit/query family, bounded history diff previews, and
full affine/default/texture conversion policy. Protocols 5 and 6 complete project/scene/entity/light/
capability authoring, general asset import/reimport and dependency/lock browsing, and versioned
host-user camera/input settings. Those operations remain named project or host boundaries rather
than a universal editor command layer.

See [the Studio migration contract](../docs/studio-migration-contract.md).
The closed adapter protocol and its explicit integration gate are documented in
[the Studio adapter protocol](../docs/studio-adapter-protocol.md).
The rationale and classification for every host/browser size ceiling is kept in
[the Studio size-limit inventory](../docs/studio-size-limit-inventory.md).

## Isolated verification

```bash
pnpm install --frozen-lockfile
pnpm run verify
```

From the repository root the equivalent explicit entry point is:

```bash
pnpm run verify:studio
```

Ordinary `./scripts/verify.sh` does not install or execute this workspace.

## Exact public package consumption

A downstream Studio composition root pins every Studio and renderer package it imports to the same
public Engine revision. The supported package spec is:

```text
github:FuzzySlipper/rusty-engine#<40-character-sha>&path:<package-path>
```

The Studio package paths are `studio/libs/adapter-client`, `studio/libs/editor-shell`,
`studio/libs/user-settings`, `studio/libs/viewport`, and `studio/libs/voxel-editor`. The renderer
paths are `render/packages/render-contracts`, `render/packages/render-projection`,
`render/packages/renderer-host`, and `render/packages/renderer-three`. A consumer declares all nine
at one exact SHA; the Studio packages expose only built `dist` entry points and use ordinary
versioned peers for this transitive closure. They contain no `workspace:`, `link:`, sibling path, or
private build dependency after installation.

After a candidate revision is public, verify that exact external installation and import surface:

```bash
./scripts/verify-studio-package-consumer.sh <40-character-public-sha>
```

This check creates a clean temporary consumer, installs all nine Git subdirectories, rejects any
local/workspace resolution in its lockfile, builds a minimal external Angular application that
imports the shell, viewport, Voxel editor, and playback components, and executes the host-neutral
adapter/settings/renderer entry points. The Angular-bearing packages publish partial-compiled
metadata so the consuming application's linker—not a source-copy workaround—finishes compilation.
Real downstream panel/product behavior remains the owning downstream repository's browser
acceptance.

## Launching the editor

Build the isolated application and start the generic Node host at one stable address:

```bash
pnpm install --frozen-lockfile
pnpm run build
pnpm run host -- --host 127.0.0.1 --port 4300
```

The host starts without a product adapter. When the user opens a project, it reads exactly one
trusted root-local `.rusty-studio.json` bootstrap, starts the declared adapter command in that
root, performs the strict `describe` handshake, and opens the selected project through a
transactional `/api/studio-session/open` request. The browser never reads the manifest and Studio
never parses the project schema. A failed build, start, handshake, or project open leaves the prior
admitted adapter/project usable where one exists. The host forwards only bounded JSON requests at
`/api/studio-adapter` to the selected adapter's closed JSONL protocol. Its separate
`/api/studio-user-settings` boundary persists versioned per-canonical-project UI, grid, and camera
preferences outside project bytes and browser storage. The default location is
`$XDG_CONFIG_HOME/rusty-engine-studio/projects` (or the platform home config directory); pass an
absolute `--settings-root` to choose another host-owned location. It does not inspect a sibling
checkout, interpret project content, or acquire gameplay authority. To open on launch, use exactly
one `root` and one project-relative `project` query parameter; the same controls remain visible in
the shell.

For exact pinned-consumer certification, retain the explicit adapter launcher or use
`pnpm run serve:den`; that path still builds and admits the configured public consumer before
listening. A root-local session is reported as `generic interactive`; an explicit
`--adapter-binary` launch without managed identity remains `unmanaged explicit adapter`.
Generic root-local discovery is a trusted development workflow, not a global registry, plugin
marketplace, schema loader, or security policy.

### Managed LAN preview

The repository root includes a `den-serve` manifest for the complete built Studio host:

```bash
den-serve up rusty-engine-studio -repo /absolute/path/to/rusty-engine
```

The launcher builds Studio and, by default, builds the adapter from the exact public reference
consumer revision in [`demo-consumer-source.json`](demo-consumer-source.json), cached outside this
repository. It never discovers or inspects a sibling checkout. The managed path does not accept a
sibling root or arbitrary prebuilt adapter override: those remain available only through the lower
level `pnpm run host` development command. Root-local discovery is reported as `generic interactive`;
an explicit prebuilt adapter override is reported as `unmanaged explicit adapter`.

Before listening, the managed host sends the strict current `describe` request, checks the configured
adapter identity and protocol, and hashes the exact adapter binary. `/health` and
`/api/studio-status` expose one frozen readout containing the Engine source commit, configured public
consumer repository/commit, running adapter build commit and binary SHA-256, and negotiated protocol.
The same compact identity appears in the title bar. Verify it before trusting a screenshot:

```bash
curl -fsS http://127.0.0.1:4300/api/studio-status | jq .
```

The supervisor watches only `demo-consumer-source.json`. If its bytes change, it emits a structured
`studioRestartRequired` receipt, terminates the complete host/adapter process group within a bounded
grace period, and exits instead of serving the stale process. Run the normal `den-serve up` command
again to build and admit the new exact consumer. `den-serve` prints the managed LAN URL and owns the
resulting process group; use `den-serve status`, `logs`, or `stop` with the same project and repository
arguments for later lifecycle operations.

The explicit real-consumer proof is separate and mutates only a temporary copy of the demo content
and conversion fixtures:

```bash
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

It starts fresh adapter processes to prove durable voxel/history/annotation/conversion/environment
state and trusted host files, then
runs visible Chromium workflows for Loading Bay and Converted Wall through the shared renderer,
including project/scene/entity/asset/settings persistence, renderer-observable brush/conversion
previews, and canonical restoration. CI
checks out the exact public demo revision declared in
[`demo-consumer-source.json`](demo-consumer-source.json). Before compilation, the gate checks that
the selected consumer's own `engine-source.json` agrees with the Engine reverse pin and invokes the
consumer-owned revision checker across Cargo, renderer, Studio, and lock carriers. Local integration
remains explicit so the ordinary Engine and isolated Studio gates never acquire a sibling-checkout
dependency.

Animated voxel-object runtime and quality proof uses a separate public consumer and pin:

```bash
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

[`voxel-consumer-source.json`](voxel-consumer-source.json) names the exact consumer and live Engine
revisions, the separate historical Engine revision that owns the checked evidence, and the baseline
runtime and high-fidelity quality reports. The gate runs the consumer-owned revision checker before
compilation, accepts only a clean checkout, copies its content into a disposable project root, and
drives normal Entity-inspector
playback through Chromium and the shared renderer. It does not make the voxel experiment an
ordinary Studio dependency or give Studio ownership of the downstream project schema.
