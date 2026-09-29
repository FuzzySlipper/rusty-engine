# Parallel lanes, round 2 (2026-09-29)

Open work across campaigns #8723 (architecture reset), #8777 (developer
experience), #8782 (wgpu renderer) and #8831 (downstream products) is split
into lanes. A lane is a list of
Den tasks that edit the same files, so they run in order inside one Claude Code
instance. Different lanes edit different files, so they run at the same time,
each in its own git worktree.

Round 1 lanes (build, content, hygiene, release, runtime, voxel-render) are
finished; their handoffs are in Git history. Start from your lane's file in
this directory. This page is the shared protocol and the ownership map.

## Lanes

| Lane | Handoff | Tasks, in order | Start |
|---|---|---|---|
| main | none, stays in the original session | review fixes for tasks whose lane has ended; coordination | now |
| tooling | [tooling.md](tooling.md) | #8808 (with #8804), #8757, #8803 (with #8774), #8801, #8809, #8816, #8802 | now |
| abi | [abi.md](abi.md) | #8744, #8799 | now; #8799's runtime-pack evidence needs tooling's #8808 |
| spatial | [spatial.md](spatial.md) | #8805, #8807, #8806 | now |
| cli | [cli.md](cli.md) | #8781, #8810, #8800 | now |
| audio | [audio.md](audio.md) | #8812, #8813, #8814 | now |
| wgpu | [wgpu.md](wgpu.md) | #8783, #8815 | now |
| wgpu-scene | [wgpu-scene.md](wgpu-scene.md) | #8784, #8788 | once #8783 publishes its resource-table layout |
| wgpu-view | [wgpu-view.md](wgpu-view.md) | #8785, #8787 | once #8783 publishes its resource-table layout |
| streaming | [streaming.md](streaming.md) | #8786 | once #8783 has readback on main |
| desktop | [desktop.md](desktop.md) | #8790, #8791 | once #8783 has surface presentation on main; #8791's decoder research can start earlier |
| downstream | [downstream.md](downstream.md) | #8832 now; then #8834, #8835, #8838, #8839, #8833, #8836, #8837 | CraftSurvive now; the rest once #8744, #8799 and #8807 land |
| playtest | [playtest.md](playtest.md) | #8765, #8769 | held by the owner; #8769 is in backlog |

Not assigned yet:
- **#8792, then #8793** (parity gate, delete Three, generate residual TS
  contracts) come after every wgpu family and the two shells. The first wgpu
  lane to finish picks them up.
- **Other campaigns.** #8718 and #8719 (playtest time and observer camera)
  belong to codex. The older campaigns #7632 and #7694 are in backlog.

Duplicates: #8804 is the same failure as #8808, and #8774 the same as #8803.
The tooling lane fixes each pair once and closes both tasks with the same
commit.

## File ownership

Edit files outside your lane only where your task truly requires it, and then
keep the edit small. If a file below belongs to another lane, expect to rebase
over its changes.

| Lane | Owns |
|---|---|
| main | nothing by default; takes returned review fixes wherever they are |
| tooling | `product-dev-host` (`host.rs`, `session.rs`, `model.rs`); `fixtures/csharp-nativeaot-trial`, `fixtures/csharp-json-persistence`; `scripts/test-runtime-pack.sh`, `scripts/audit-standalone.sh` and the evidence manifests it checks; `render/browser/renderer.browser.spec.ts`; one-line clippy fixes anywhere (#8757) |
| abi | `csharp-engine-abi` (except `world_origin.rs` and `perception.rs`); lease and result plumbing in `csharp-engine-services`; `csharp/Rusty.Engine.BindingGenerator`, `csharp/Rusty.Engine.ProductGenerator`; `csharp-product-runtime/src/lib.rs` product binding (`from_bound_product`, `optional_callback_pair`); `csharp/Rusty.Engine/NativeProduct/ProductBridge.cs`; `generated_abi_identity.rs` |
| spatial | `csharp-engine-services/src/perception.rs`, `spatial.rs` (`entity_state`), `world_origin.rs`; `csharp-engine-abi/src/world_origin.rs`, `perception.rs`; `engine-spatial/src/perception.rs`, `world_origin.rs`, `character_controller.rs`; `EntityOriginRebaser.cs`, `EntityCharacterController.cs` |
| cli | `rusty-cli`; `/home/agent/dev/rusty-template`; the `docs/csharp-sdk.md` split; downstream pins, installer scripts and project files in Dagger, CraftSurvive and the #8810 products |
| audio | `render-audio`; `csharp-product-runtime/src/audio_output.rs`; `docs/recorded-audio.md`; Doom's audio policy (`LoadingBayWorldServices.cs`) |
| wgpu | `render-wgpu` crate root, device, tables and pass pipeline; `rust/prototypes/wgpu-bootstrap`; the wgpu row in `scripts/dependency_boundary_check.py` |
| wgpu-scene | `render-wgpu` modules for meshes, voxel surfaces, lighting, shadows, sky, ghost plates, animated meshes, telemetry |
| wgpu-view | `render-wgpu` modules for cameras, view composition, viewmodel, multi-view, captures, billboards, sprites, particles |
| streaming | the runtime's frame stream endpoint; the browser shell canvas in `render/packages/product-browser-host` |
| desktop | the new desktop shell crate; the video realizer |
| downstream | the product repositories (CraftSurvive, Dungeon, Underworld, D20, Roguelike, Crawler, Rifles, Space); `docs/evidence/downstream-*`; no Engine source |
| playtest | `docs/evidence/` for its tasks; `render-presentation/src/video.rs`; the TS video host |

Known overlaps:
- **`csharp-engine-services/src/dynamics.rs`.** abi (if #8744 starts with
  Dynamics) and spatial (#8807 sets `expected_origin_revision` there). Spatial
  keeps its edit to that one field.
- **Generated bindings.** abi and spatial both change the ABI. Regenerate on
  conflict; see below.
- **`render-wgpu`.** The wgpu lane owns the crate root and the shared tables.
  Family lanes add modules and extend the tables through the layout #8783
  publishes. A table change goes to Den first.
- **`docs/csharp-sdk.md`.** cli splits it in one commit (#8781). Everyone else
  edits only their own section, and keeps other lanes' sections on conflict.

## Protocol for every lane

1. **Worktree.** `/home/agent/dev/worktrees/re-<lane>` on branch `lane/<lane>`.
   - **If the directory already exists** (round 1 left spatial, cli, wgpu and
     audio), check that `git status` is clean, then run
     `git fetch origin && git switch -C lane/<lane> origin/main` in it. Its
     build cache is reused.
   - **Otherwise** run
     `git worktree add /home/agent/dev/worktrees/re-<lane> -b lane/<lane> origin/main`.

   Work there only. Never edit `/home/agent/dev/rusty-engine`; the main lane
   uses it.
2. **Den.** Project `rusty-engine`; sender `claude-code-<lane>`.
   - Before any `update_task`, re-read the task: a description update replaces
     the whole text, and other agents edit tasks concurrently.
   - Set the task `in_progress` when you start it.
3. **Read first.** `AGENTS.md` (the campaign #8723 section overrides older
   preservation language), the Den doc named by your campaign, and your task,
   including its scope updates.
4. **Implement.** Removal-first, as the campaign charter says. Put evidence in
   `docs/evidence/<topic>-<task>/README.md`. Run the checks that answer the
   task's question: focused tests, clippy on the crates you touched, and the
   SDK smoke or `scripts/test-runtime-pack.sh` when you change the ABI,
   generator or C# SDK.
5. **Land each task on main separately:**
   - `git fetch origin && git rebase origin/main`;
   - rerun your focused checks;
   - `git push origin HEAD:main`. If the push is rejected, rebase and push
     again. Never force-push main.
6. **Review request.** Post a Den message on the task with intent
   `review_request`, metadata `{"type":"review_request","commit":"<sha>"}`, and
   a short account: what was removed, what was kept and why, the evidence,
   migration notes, and follow-ups. Then set the task to `review` and start
   your next task. Review feedback that arrives while your lane is running is
   yours to fix. Once your lane has ended, the main lane takes it.
7. **Follow-ups.** File each one as an explicit Den task under the campaign
   before you post the review request, and name it in the message.
8. **Commit messages** have no `Co-Authored-By` or other attribution lines.

## Shared generated files

- **`render/artifacts/*`** and `rust/crates/renderer-webview-host/artifacts/`
  are ignored build output since #8752. Do not commit them. `pnpm run build`
  in `render/` rebuilds them, and `scripts/build-runtime-pack.sh` builds them
  itself.
- **`rust/crates/csharp-engine-abi/src/generated_abi_identity.rs`.** On
  conflict, regenerate it with `scripts/generate-csharp-native-bindings.sh`
  (building the SDK also regenerates it).
- **`Cargo.lock`, `pnpm-lock.yaml`.** Take one side, then let cargo or pnpm
  resolve it.
- **Heavily shared docs** (`docs/csharp-sdk.md`, `docs/architecture.md`). Edit
  only the sections your task changes; on conflict keep both sides' edits.

## Known baseline

- **Tests.** `cargo test --workspace --exclude renderer-webview-host` passes on
  main (1,145 tests at `a3b43343`).
  - The GTK/WebKit dev packages are now installed, so `renderer-webview-host`
    builds too; its check script needs `pnpm --dir render run build:webview-artifact` first.
- **Clippy.** It still reports the #8757 lints in `product-dev-host/src/model.rs`,
  `render-presentation/src/frame.rs` and `csharp-engine-services`
  (`spatial.rs`, `voxel.rs`) until the tooling lane lands #8757. They are not
  yours.
- **`scripts/test-runtime-pack.sh`** fails at the NativeAOT fixture (#8808)
  until tooling fixes it. Say so in your evidence rather than skipping
  silently.
- **`scripts/test-csharp-sdk-package.sh`** (`--coreclr-smoke`, `--aot`) passes.
  It needs `DOTNET_ROOT=/home/agent/.dotnet`, and a `TMPDIR` whose path does not
  start with the checkout's path.
- **Temp space.** `/tmp` is a shared tmpfs with a per-user quota, and it has
  filled up twice. Put large scratch builds in your worktree or your session
  scratchpad, and delete them when you are done.
- **Downstream.** Products pin their SDK. When your change breaks a product
  API, write migration notes; follow AGENTS.md on downstream pairs.
  Downstream checkouts: `/home/agent/dev/rusty-dagger`,
  `/home/agent/dev/rusty-doom`, `/home/agent/dev/rusty-craftsurvive`,
  `/home/agent/dev/rusty-template`.
