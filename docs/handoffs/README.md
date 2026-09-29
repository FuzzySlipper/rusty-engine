# Parallel lanes, 2026-09-29

Open work across campaigns #8723 (architecture reset), #8777 (developer
experience) and #8782 (wgpu renderer) is split into lanes. A lane is a list of
Den tasks that edit the same files, so they run in order inside one Claude Code
instance. Different lanes edit different files, so they run at the same time,
each in its own git worktree.

Start from your lane's file in this directory. This page is the shared
protocol and the ownership map.

## Lanes

| Lane | Handoff | Tasks, in order | Start |
|---|---|---|---|
| main | none, stays in the original session | review fixes for #8736, #8737, #8739, #8742; then #8798 (with #8759); then #8771 | now |
| spatial | [spatial.md](spatial.md) | #8754 (with #8755) | now |
| voxel-render | [voxel-render.md](voxel-render.md) | #8797 | now |
| build | [build.md](build.md) | #8775, #8776, #8764 | now |
| runtime | [runtime.md](runtime.md) | #8753, #8772, #8745, #8770 | now; #8770 after the #8736 fix lands |
| content | [content.md](content.md) | #8743, #8763 | now |
| hygiene | [hygiene.md](hygiene.md) | #8794, #8752 | now |
| release | [release.md](release.md) | #8778, #8780 | now |
| cli | [cli.md](cli.md) | #8779, #8781 | after #8775 is on main |
| wgpu | [wgpu.md](wgpu.md) | #8796, #8783 | after the #8795 survey document exists |
| audio | [audio.md](audio.md) | #8789 | now |
| playtest | [playtest.md](playtest.md) | #8765, #8769 | deferred by the owner; #8769 is in backlog |

Not assigned yet:
- **#8744** (result buffers) sweeps every service's result API. It starts after
  the service lanes (main, spatial, voxel-render) are quiet.
- **wgpu families #8784, #8785, #8787, #8788, #8790 and #8791** start once #8783
  publishes its resource-table layout.
- **#8786** starts once #8783 has readback.
- **#8792, then #8793** come after all of the above.

## File ownership

Edit files outside your lane only where your task truly requires it, and then
keep the edit small. If a file below belongs to another lane, expect to rebase
over its changes.

| Lane | Owns |
|---|---|
| main | `csharp-product-runtime/src/lib.rs` fault and publication paths (`fault_after_call`); `render-projection/src/runtime_appearance.rs` and `appearance.rs`; `csharp-engine-services/src/appearance.rs`, `content/`, `authored_content.rs`, `persistence.rs`; `engine-spatial/src/lib.rs` mesh neighbourhood and `voxel_edit.rs`; `product-dev-host/src/host.rs` and `model.rs`; `render-presentation` descriptors; `product-browser-host/src/local-transport.ts` |
| spatial | `csharp-engine-services/src/spatial.rs`, `world_origin.rs`, `kinematic.rs`, character receipts in `csharp-engine-abi`; `engine-spatial/src/trigger.rs`, `world_origin.rs`; `svc-collision/src/static_mesh.rs` guards; `csharp/Rusty.Engine/Entities/EntityKinematicMotion.cs` |
| voxel-render | `render-projection/src/voxel.rs`; `csharp-engine-services/src/voxel_scene_presentation.rs` |
| build | `csharp/Rusty.Engine/buildTransitive/Rusty.Engine.targets`; `csharp/Rusty.Engine.ProductGenerator/`; `csharp/Rusty.Engine/Rusty.Engine.csproj`; `rusty-cli` `stage_product` |
| runtime | `csharp-product-runtime/src/main.rs` and `supervisor.rs`; `runtime-session`; `runtime-ui`; the test-only `render-projection` modules (`entity.rs`, `debug.rs`, `model_preview.rs`, `RetainedNodeProjector`); `engine-inspector` |
| content | source-root content and UI paths; `content-store`, `content-store-host`, `asset-import`, `authored-scene`; the `rusty dev` reload path |
| hygiene | `studio/`; `.github/workflows/studio.yml`; `scripts/check-ci-routing.py`; `docs/README.md`; tracked build outputs |
| release | pair build/publish scripts and the CI workflow that publishes pairs |
| cli | `rusty-cli` install, update and status commands; `rusty-template` |
| wgpu | the new `render-wgpu` crate; the dependency boundary check |
| audio | the new Rust audio realization crate |
| playtest | `docs/evidence/` for its tasks; `render-presentation/src/video.rs`; the TS video host |

`render-projection/src/retained.rs` (`StableHandleRegistry`) is shared: main
uses it and runtime deletes `RetainedNodeProjector` next to it. Delete only the
projector.

## Protocol for every lane

1. **Worktree.** `git worktree add /home/agent/dev/worktrees/re-<lane> -b lane/<lane> origin/main`,
   then work there only. Never edit `/home/agent/dev/rusty-engine`; the main
   lane uses it.
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
   your next task. Review feedback returns the task to `in_progress`; fix it in
   your lane.
7. **Follow-ups.** File each one as an explicit Den task under the campaign
   before you post the review request, and name it in the message.
8. **Commit messages** have no `Co-Authored-By` or other attribution lines. The
   owner rewrote main to remove them.

## Shared generated files

- **`render/artifacts/*`** (browser bundles). Rebuild them with
  `pnpm run build` in `render/` only if your lane changed TypeScript. On a
  rebase conflict, take either side and rebuild; never hand-merge minified
  output. #8752 may stop tracking them.
- **`rust/crates/csharp-engine-abi/src/generated_abi_identity.rs`.** On
  conflict, regenerate it with `scripts/generate-csharp-native-bindings.sh`
  (building the SDK also regenerates it).
- **`Cargo.lock`, `pnpm-lock.yaml`.** Take one side, then let cargo or pnpm
  resolve it.
- **Heavily shared docs** (`docs/csharp-sdk.md`, `docs/architecture.md`). Edit
  only the sections your task changes; on conflict keep both sides' edits.

## Known baseline

- Clippy reports pre-existing lints in `content-store`, `render-presentation`
  and `csharp-engine-services/src/spatial.rs` (#8757). These are not yours;
  don't claim them.
- The workspace tests run with
  `cargo test --workspace --exclude renderer-webview-host`, because that crate
  needs GTK.
- Downstream products pin their SDK. When your change breaks a product API,
  write migration notes; follow AGENTS.md on downstream pairs.
