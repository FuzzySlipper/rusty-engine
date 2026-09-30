# Parallel lanes

When several agents work on Rusty Engine at once, the work is split into
lanes. A lane is a list of Den tasks that edit the same files, so they run in
order inside one agent. Different lanes edit different files, so they run at
the same time, each in its own git worktree. The main checkout,
`/home/agent/dev/rusty-engine`, belongs to the coordinating session.

A round of lanes is planned in Den: the campaign or coordination task lists
each lane's tasks, their order, the files it owns, and known overlaps. A lane
may get a short handoff file here for the round; delete it when the lane ends.

## Protocol for every lane

1. **Worktree.** `/home/agent/dev/worktrees/re-<lane>` on branch `lane/<lane>`.
   - **If the directory already exists**, check that `git status` is clean,
     then run `git fetch origin && git switch -C lane/<lane> origin/main` in
     it. Its build cache is reused.
   - **Otherwise** run
     `git worktree add /home/agent/dev/worktrees/re-<lane> -b lane/<lane> origin/main`.

   Work there only. Never edit the main checkout.
2. **Den.** Project `rusty-engine`; sender `claude-code-<lane>`.
   - Before any `update_task`, re-read the task: a description update replaces
     the whole text, and other agents edit tasks concurrently.
   - Set the task `in_progress` when you start it.
3. **Read first.** `AGENTS.md`, the Den documents your campaign names, and
   your task, including its scope updates.
4. **Implement.** Run the checks that answer the task's question: focused
   tests, clippy on the crates you touched, and the SDK smoke or
   `scripts/test-runtime-pack.sh` when you change the ABI, generator or C# SDK.
   Evidence goes in Den (the review request, or a Den document for anything
   long), not in the repository. A product-facing behaviour change gets a
   `Migration:` section in its commit message
   ([how](../csharp-distribution.md)).
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
   yours to fix. Once your lane has ended, the main session takes it.
7. **Follow-ups.** File each one as an explicit Den task under the campaign
   before you post the review request, and name it in the message.
8. **Commit messages** have no `Co-Authored-By` or other attribution lines.

## File ownership

Edit files outside your lane only where your task truly requires it, and then
keep the edit small. If a file belongs to another lane, expect to rebase over
its changes. Heavily shared docs (`docs/csharp-sdk.md`, `docs/architecture.md`):
edit only the sections your task changes; on conflict keep both sides' edits.

## Shared generated files

- **`render/artifacts/*`** are ignored build output. Do not commit them.
  `pnpm run build` in `render/` rebuilds them, and
  `scripts/build-runtime-pack.sh` builds them itself.
- **`rust/crates/csharp-engine-abi/src/generated_abi_identity.rs`.** On
  conflict, regenerate it with `scripts/generate-csharp-native-bindings.sh`
  (building the SDK also regenerates it).
- **`Cargo.lock`, `pnpm-lock.yaml`.** Take one side, then let cargo or pnpm
  resolve it.
- **`render/packages/*/src/generated/contracts.ts`.** Regenerate with
  `scripts/generate-typescript-contracts.sh`.

## Environment

- **SDK smoke.** `scripts/test-csharp-sdk-package.sh` (`--coreclr-smoke`,
  `--aot`) needs `DOTNET_ROOT=/home/agent/.dotnet`, and a `TMPDIR` whose path
  does not start with the checkout's path.
- **Temp space.** `/tmp` is a shared tmpfs with a per-user quota. Put large
  scratch builds in your worktree or your session scratchpad, and delete them
  when you are done.
- **Downstream.** Products pin their SDK pair. When your change breaks a
  product API, write a migration note; follow AGENTS.md on downstream pairs.
  Downstream checkouts live under `/home/agent/dev/` (`rusty-dagger`,
  `rusty-doom`, `rusty-craftsurvive`, `rusty-template` and the others).
