# Rusty Engine Studio

This is the isolated first-party Studio workspace. It will reach full useful feature parity with
the pinned Asha Studio donor while consuming Rusty Engine's canonical Rust owners and shared
renderer.

M11A contains the source inventory, dispositions, owner-adoption map, dependency boundary, and
workspace gate. M11B adds the structural TypeScript client for the real project-owned Loading Bay
Rust adapter before Angular product code is imported in M11C. Scene/viewport and complete
voxel/material/conversion workflows follow in M11D and M11E. Until the shell lands this directory is
an implemented boundary workspace, not yet a launchable editor.

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

The explicit real-consumer proof is separate:

```bash
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

CI checks out the exact public demo revision declared in
[`demo-consumer-source.json`](demo-consumer-source.json). Local integration remains explicit so the
ordinary Engine and isolated Studio gates never acquire a sibling-checkout dependency.
