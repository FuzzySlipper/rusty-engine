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

## Launching the editor

Build the isolated application, build a downstream project's adapter, and start the explicit Node
host with an absolute adapter path:

```bash
pnpm install --frozen-lockfile
pnpm run build
pnpm run host -- --adapter-binary /absolute/path/to/studio-adapter
```

The host serves the built application and forwards only bounded JSON requests at
`/api/studio-adapter` to that adapter's closed JSONL protocol. Its separate
`/api/studio-user-settings` boundary persists versioned per-canonical-project UI, grid, and camera
preferences outside project bytes and browser storage. The default location is
`$XDG_CONFIG_HOME/rusty-engine-studio/projects` (or the platform home config directory); pass an
absolute `--settings-root` to choose another host-owned location. It does not inspect a sibling
checkout, interpret project content, or acquire gameplay authority. To open on launch, use exactly
one `root` and one project-relative `project` query parameter; the same controls remain visible in
the shell.

### Managed LAN preview

The repository root includes a `den-serve` manifest for the complete built Studio host:

```bash
den-serve up rusty-engine-studio -repo /absolute/path/to/rusty-engine
```

The launcher builds Studio and, by default, builds the adapter from the exact public reference
consumer revision in [`demo-consumer-source.json`](demo-consumer-source.json), cached outside this
repository. It never discovers or inspects a sibling checkout. To select a consumer explicitly,
set `RUSTY_STUDIO_CONSUMER_ROOT` to its absolute checkout path; to use an already-built adapter, set
`RUSTY_STUDIO_ADAPTER_BINARY` to its absolute path. `den-serve` prints the managed LAN URL and owns
the resulting process group; use `den-serve status`, `logs`, or `stop` with the same project and
repository arguments for later lifecycle operations.

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
[`demo-consumer-source.json`](demo-consumer-source.json). Local integration remains explicit so the
ordinary Engine and isolated Studio gates never acquire a sibling-checkout dependency.

Animated voxel-object runtime and quality proof uses a separate public consumer and pin:

```bash
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

[`voxel-consumer-source.json`](voxel-consumer-source.json) names the exact consumer and Engine
revisions plus the baseline runtime and high-fidelity quality reports. The gate accepts only a clean
checkout, copies its content into a disposable project root, and drives normal Entity-inspector
playback through Chromium and the shared renderer. It does not make the voxel experiment an
ordinary Studio dependency or give Studio ownership of the downstream project schema.
