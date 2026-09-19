# Lane: Existing owner reuse

**Always on.** Run this lane on every task.

## One question

Does this change create a competing mechanism instead of extending this
repository's existing owner for that behavior?

## Why

The same failure as capability reinvention, one level down. An agent working
from a task description will add a new service, store, or state holder without
noticing that this repository already owns that concept. The result is two
owners for one concept, which is worse than an absent feature because both look
correct in isolation.

## Basis required for an actionable finding

Name all four:

1. the existing owner in this repository, by file and type or member;
2. the new duplicate, by file and line;
3. the overlapping state, behavior, or authority the two share;
4. the consequence — which callers now disagree, or which invariant can no
   longer hold — and the relevant callers by name.

Search before concluding: the existing owner may live in `rust/crates/*`,
`csharp/Rusty.Engine/`, `csharp/Rusty.Engine.ProductGenerator/`, or `studio/`
depending on what the concept is. Read `AGENTS.md` and the source-owners table
in `docs/architecture.md` rather than assuming from a directory name.

## Not a finding

- A new file, or a similar name. Overlap must be shown in state or behavior.
- Extension of an existing owner that happens to add a type, member, or file.
  That is the desired outcome of this lane.
- A deliberate second implementation that the task explicitly calls for, for
  example a canary that must not share the production path.
