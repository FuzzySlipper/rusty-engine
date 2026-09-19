# Lane: Existing capability reuse

**Always on.** Run this lane on every task.

## One question

Does this change recreate a mechanism that existing Engine code already owns?

## Why

An agent that cannot find a capability will build one. Upstream that produces
a second implementation of something the Engine already guarantees — a second
spatial authority, a second renderer or retained-frame path, a parallel
lifecycle, a duplicate persistence or ABI seam — which then drifts, and it
hides the real signal: either the existing owner should be extended, or the
need is genuinely new and deserves one coherent named capability through the
`csharp-engine-abi` / `csharp-engine-services` / generator path. A duplicate
here is worse than downstream duplication because every product must reconcile
with both authorities.

## Basis required for an actionable finding

Name all four:

1. the existing owner in this repository, by crate or project, file, and type
   or member;
2. the new duplicate the change introduces, by file and line;
3. the overlapping state, behavior, or authority the two share;
4. the consequence — which callers now disagree, or which invariant can no
   longer hold — and the concrete adoption path: extend the owner, or add one
   named capability through the ABI/services/generator route.

A finding that names only "this looks like something the Engine might own" is
not actionable. Verify the contract in the actual crate or generated safe C#
surface before claiming it exists.

Search before concluding: the existing owner may live in `rust/crates/*`,
`csharp/Rusty.Engine/`, `csharp/Rusty.Engine.ProductGenerator/`, or `studio/`
depending on what the concept is. Read `AGENTS.md` and the source-owners table
in `docs/architecture.md` rather than assuming from a directory name.

## Not a finding

- A genuinely new coherent capability with no existing owner. That is an
  addition, not a duplicate — say what you searched and move on.
- A local helper that is genuinely narrower than the Engine mechanism and does
  not duplicate its guarantees.
- Extension of an existing owner that happens to add a type, member, or file.
  That is the desired outcome of this lane.
- A deliberate second implementation that the task explicitly calls for, for
  example a canary that must not share the production path.
