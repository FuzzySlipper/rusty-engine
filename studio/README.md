# Rusty Engine Studio

This is the isolated first-party Studio workspace. It will reach full useful feature parity with
the pinned Asha Studio donor while consuming Rusty Engine's canonical Rust owners and shared
renderer.

M11A contains the source inventory, dispositions, owner-adoption map, dependency boundary, and
workspace gate. The real external-project Rust adapter lands in M11B before Angular product code is
imported in M11C. Scene/viewport and complete voxel/material/conversion workflows follow in M11D and
M11E. Until then this directory is intentionally an architecture and verification skeleton, not a
launchable editor.

See [the Studio migration contract](../docs/studio-migration-contract.md).

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
