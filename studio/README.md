# Rusty Engine Studio

This is the isolated first-party Studio workspace. It will reach full useful feature parity with
the pinned Asha Studio donor while consuming Rusty Engine's canonical Rust owners and shared
renderer.

M11A contains the source inventory, dispositions, owner-adoption map, dependency boundary, and
workspace gate. M11B supplies the structural TypeScript client for the real project-owned Loading
Bay Rust adapter. M11C supplies the Angular/Nx editor shell. M11D makes canonical authored-scene
hierarchy, typed transform settlement, and shared-renderer lifecycle/grid/picking operational.
M11E adds project-embedded voxel assets and transformed instances, material/palette authoring,
Rust-revalidated voxel picking, atomic brush edits, durable history, typed annotations, bounded
model queries, and private-plan GLB conversion through the same renderer. M11F owns the final donor
parity reconciliation recorded in the migration contract and owner-adoption map.

See [the Studio migration contract](../docs/studio-migration-contract.md).
The closed adapter protocol and its explicit integration gate are documented in
[the Studio adapter protocol](../docs/studio-adapter-protocol.md).

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
`/api/studio-adapter` to that adapter's closed JSONL protocol. It does not inspect a sibling
checkout, interpret project content, or acquire gameplay authority. To open on launch, use exactly
one `root` and one project-relative `project` query parameter; the same controls remain visible in
the shell.

The explicit real-consumer proof is separate and mutates only a temporary copy of the demo content
and conversion fixtures:

```bash
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

It starts fresh adapter processes to prove durable voxel/history/annotation/conversion state, then
runs visible Chromium workflows for Loading Bay and Converted Wall through the shared renderer. CI
checks out the exact public demo revision declared in
[`demo-consumer-source.json`](demo-consumer-source.json). Local integration remains explicit so the
ordinary Engine and isolated Studio gates never acquire a sibling-checkout dependency.
