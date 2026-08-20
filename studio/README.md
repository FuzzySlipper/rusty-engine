# Rusty Engine Studio

This is the isolated first-party Studio workspace. It targets full useful feature parity with
the historical Asha Studio donor while consuming Rusty Engine's canonical Rust owners and shared
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

## Package and host ownership

Studio and the renderer are isolated Engine-owned workspaces. Verify this workspace with
`pnpm run verify:studio`; a downstream Rust game does not install, import, build, or configure
these packages as a second Studio or renderer authority.

Downstream projects consume the complete Rust facade through one unconditional sibling path
dependency and submit retained facts through the Engine-owned Rust renderer/webview or
`@rusty-engine/application-host` boundary. The Engine-hosted Studio discovers a project through its
root-local `.rusty-studio.json`, project data, and the project's Rust adapter. It does not require a
downstream copy of the Studio shell, renderer TypeScript, Three/WebGL backend, private bridge, or
child HTML document.

When a selected downstream checkout needs focused adapter or browser proof, use the explicit
integration gate for that checkout. Exact source and consumer commits belong in the Den task or
review evidence for that run; they are not package pin files or a repository-wide freshness
contract. The ordinary Engine and isolated Studio gates do not launch every downstream checkout.

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

For normal development, the operator selects the checkout, project root, and adapter explicitly.
Studio consumes that checkout as it stands; it does not fetch, pull, reset, clean, checkout, pin, or
enforce a downstream Engine freshness policy. A root-local session is reported as
`generic interactive`; an explicit `--adapter-binary` launch without managed identity remains
`unmanaged explicit adapter`.
Generic root-local discovery is a trusted development workflow, not a global registry, plugin
marketplace, schema loader, or security policy.

### Managed LAN preview

The repository root includes a `den-serve` manifest for the complete built Studio host:

```bash
den-serve up rusty-engine-studio -repo /absolute/path/to/rusty-engine
```

The launcher builds Studio and starts one selected adapter/project session. It does not discover,
fetch, or mutate a sibling checkout, and it does not turn an operational source identity into a
dependency pin. Root-local discovery is reported as `generic interactive`; an explicit prebuilt
adapter override is reported as `unmanaged explicit adapter`.

Before listening, the managed host sends the strict current `describe` request, checks the selected
adapter identity and protocol, and hashes the running adapter binary. `/health` and
`/api/studio-status` expose one frozen operational readout containing the selected source/build
identities, adapter binary SHA-256, and negotiated protocol. The same compact identity appears in the
title bar. Verify it before trusting a screenshot:

```bash
curl -fsS http://127.0.0.1:4300/api/studio-status | jq .
```

The supervisor watches the selected session inputs. If they change, it emits a structured
`studioRestartRequired` receipt, terminates the complete host/adapter process group within a bounded
grace period, and exits instead of serving stale state. Run the normal `den-serve up` command again
to admit the selected checkout and project. `den-serve` prints the managed LAN URL and owns the
resulting process group; use `den-serve status`, `logs`, or `stop` with the same project and
repository arguments for later lifecycle operations.

Animated voxel-object runtime and quality proof uses a separate explicit consumer checkout:

```bash
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

The gate accepts a selected checkout, copies its content into a disposable project root, and drives
normal Entity-inspector playback through Chromium and the shared renderer. Exact source heads and
quality reports are run evidence, not a downstream dependency pin. It does not make the voxel
experiment an ordinary Studio dependency or give Studio ownership of the downstream project schema.
