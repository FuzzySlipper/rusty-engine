# Lane: content

**Tasks, in order:** #8743, #8763. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Survey #8762 describes the real consumers.
Shared protocol: [README.md](README.md).

## Tasks

- **#8743: source-root content and UI reload without replacing the runtime.**
  - Since #8766 the runtime serves HTTP/SSE itself on a listener inherited
    from the supervisor. `rusty dev` can only send `replace-runtime` today, so
    a non-code reload needs a route to the running runtime. Two options:
    - a new supervisor command forwarded to the runtime;
    - `rusty dev` calling a runtime HTTP endpoint directly.

    Pick the smaller one and record it on the task. The cli lane (#8779)
    reads it.
  - Build isolation (owner-approved) is part of this task: UI edits skip
    managed compilation, and C#-only edits skip unchanged UI compilation. That
    includes migrating the `tsc` hooks in Dagger's `WorldRpg.Host.csproj` and
    `CraftSurvive.Game.csproj`.
- **#8763: simplify content storage and import around actual consumers.** It
  now also owns the fate of `content-store` and `content-store-host`: delete
  them, or keep them with a named consumer. Studio, their likely user, is being
  deleted by #8794.

## Coordination

- **build lane (#8775, #8776, #8764)** owns `Rusty.Engine.targets` and the
  nested build. Your build-isolation work meets theirs at the target and
  command boundary, so agree the shape in Den before editing the same target.
  Source-root reload itself does not wait for them.
- **main lane (#8798)** absorbed #8759, the open-time SHA-256 re-hash of every
  bundle file in `csharp-engine-services/src/content/bundles.rs`
  `ProductContentBundles::load`. Leave that verification code to the main lane.
- **cli lane (#8779)** changes runtime resolution in `rusty dev` after #8775.
  Your reload route is the part they read.
- **#8742 (`25dfd514`)** removed the 1 MiB content-store read cap.

## Files

- **Owns:**
  - source-root content and UI serving;
  - the `rusty dev` reload route;
  - `content-store`, `content-store-host`, `asset-import`, `authored-scene`;
  - the content admission code in `csharp-engine-services/src/content/`,
    except `bundles.rs` verification.
- **Leave alone:** `authored_content.rs` caps and `persistence.rs`. Those are
  main lane, under #8798.

## Evidence

Follow each task's acceptance:
- a UI edit and a content edit with no managed restart;
- old and new live references;
- deletion and an import failure;
- the actual compiler invocations for unchanged, C#-only and UI-only edits.
