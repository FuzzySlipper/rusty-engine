# Downstream Engine revision contract

## Purpose

This topic defines how an external consumer selects and updates Rusty Engine
without a sibling checkout, mixed package revisions, or a provider-owned release
system. It applies when a consumer uses any combination of Engine Rust crates,
renderer packages, Studio packages, or rules-authoring packages from the public
repository.

The contract is deliberately small:

> One consumer owns one exact public Rusty Engine commit for every Engine surface
> it selects.

Consumers may advance independently. A game, a voxel experiment, and another
product do not need to use the same Engine commit. Within one consumer, however,
Rust crates and TypeScript packages from this repository stay on the same commit.
Keeping separate Rust, renderer, Studio, or rules lanes would make compatibility
an implicit matrix and recreate the drift this contract is meant to remove.

## Source manifest

The consumer keeps `engine-source.json` at its repository root. These fields form
the common identity:

```json
{
  "schemaVersion": 1,
  "publicRepository": "https://github.com/FuzzySlipper/rusty-engine",
  "commit": "<40 lowercase hexadecimal characters>"
}
```

- `schemaVersion` is exactly `1` for this shape.
- `publicRepository` is the canonical public HTTPS repository. A local path,
  sibling checkout, private mirror, or alternate remote is not an equivalent
  source.
- `commit` is exactly 40 lowercase hexadecimal characters and must be fetchable
  from the public repository before an update is accepted. Branches, tags,
  abbreviated hashes, and symbolic refs are rejected.

A consumer may add narrowly owned fields, such as the directory used by its
managed Studio launcher. Its decoder still admits an explicit schema and rejects
unknown fields; extensions do not create another Engine revision.

The manifest is the editable source of truth. Cargo manifests, package manifests,
package-manager policy, and lockfiles necessarily repeat its commit as checked
projections because those tools require literal dependency sources.

## Keep the identities separate

An Engine commit is not a protocol version or a certification result:

| Identity | Owner | Meaning |
|---|---|---|
| Consumer Engine commit | downstream consumer | Provider tree supplying all selected Engine crates and packages |
| Studio adapter protocol version | concrete downstream adapter and Studio client | Typed request and response compatibility |
| Product transport protocol version | concrete product host and client | Product session compatibility |
| Reverse-certification consumer commit | Rusty Engine integration fixture | Exact downstream commit selected for an Engine-owned integration check |
| Reverse-certification `engineCommit` | Rusty Engine integration fixture | Engine commit that the selected downstream commit claims to consume |
| Historical provenance or evidence commit | owner of the record | Commit that produced an earlier transfer, conversion, benchmark, or proof |

Changing `engine-source.json` never mechanically changes a Studio adapter or
product transport protocol number. A protocol changes only when its typed
contract changes and its owner deliberately versions it.

Engine owns files such as `studio/demo-consumer-source.json` and
`studio/voxel-consumer-source.json`. They are reverse-certification targets: each
names an exact consumer commit and the Engine commit that consumer claims. A
consumer revision command must never mutate them. Engine updates a reverse pin in
a later Engine commit after the final reviewed downstream commit exists.

Historical provenance, checked evidence, and arbitrary 40-character test values
are not live dependency carriers. An update must not rewrite them merely because
they happen to contain the old commit.

## Consumer-owned command surface

Each consumer provides the same visible modes while keeping carrier knowledge in
the repository that owns those manifests:

```text
./scripts/engine-revision check
./scripts/engine-revision update <40-character-public-sha>
./scripts/engine-revision update <40-character-public-sha> --dry-run
```

The implementation language is consumer-owned. Rusty Engine does not reach into
another repository, ship a cross-repository mutator, or maintain a registry of
every consumer's files.

### `check`

`check` is deterministic, read-only, and network-independent. It:

1. strictly decodes `engine-source.json` and validates its repository and commit;
2. verifies every active Cargo Git dependency uses the manifest commit and the
   canonical public repository;
3. verifies every locked Engine Cargo package resolves that same source and
   commit;
4. verifies every selected renderer, Studio, or rules package manifest uses the
   same commit and its declared package path;
5. verifies package-manager build policy and lock resolution use the same commit;
6. rejects path, sibling, branch, tag, floating, mixed, missing, unexpected, and
   stale-lock sources; and
7. reports the owning file, expected commit, observed value, and repair command.

Local audits and runtime provider readouts derive their expected identity from the
source manifest instead of adding another manually updated constant. The checker
knows its consumer's active carriers and can reject newly introduced Engine
dependencies that were not added to that local accounting.

### `update`

`update` validates a full commit and proves it is fetchable from the canonical
public repository. It then:

1. refuses changes in active carrier or lockfile paths while preserving unrelated
   worktree changes;
2. builds the candidate in a disposable checkout or worktree at the caller's
   exact `HEAD`;
3. rewrites only the source manifest and the consumer's declared active
   projections;
4. regenerates each Cargo or pnpm lockfile the consumer owns with its
   repository-pinned tool version;
5. runs `check` and the focused dependency or boundary checks in the candidate;
6. captures the exact scoped diff; and
7. rechecks the caller before applying that validated diff.

Failure cleans the candidate and leaves the caller's active files unchanged. A
successful command changes files and prints their diff; it does not commit, push,
edit task state, update an Engine reverse pin, or run unrelated product policy.

`--dry-run` performs the same candidate generation and validation, prints the
prospective diff, and removes the candidate without changing the caller.

## Active carriers and generated locks

Active carriers are the files a consumer must change to select its current
provider, commonly:

- Cargo dependency `rev` values and intentional Cargo metadata;
- root or package-level Engine package dependencies;
- pnpm `allowBuilds` keys for exact codeload package paths;
- `Cargo.lock` and `pnpm-lock.yaml`; and
- runtime or audit readouts that must report the current provider identity.

Lockfiles are generated evidence, not independent revision choices. The update
command regenerates them and `check` verifies every resolved Engine source; it
does not hand-edit one convenient occurrence while leaving nested resolutions
stale.

Do not use repository-wide textual replacement. Source-provenance documents,
conversion reports, benchmark results, retained evidence, donor commits, and
synthetic test hashes describe a different fact. Active prose should point to
`engine-source.json` instead of repeating a value that must move with every
update.

## Migration and rollback

To adopt the contract in an existing consumer:

1. inventory every active manifest, policy, lock, audit, and runtime copy of the
   current Engine commit;
2. classify historical and synthetic occurrences so they are protected from the
   updater;
3. add `engine-source.json` and the consumer-owned command;
4. make avoidable audit and runtime constants derive from the manifest;
5. run `update` with the currently selected commit to normalize all projections;
6. run `check`, the consumer's standalone gate, and any real product, browser, or
   Studio integration that owns affected behavior; and
7. commit the coherent manifest and generated-lock change together.

Rollback uses the same command with the prior exact public commit:

```text
./scripts/engine-revision update <prior-provider-sha>
```

Exercise rollback in a disposable checkout. Reverting an isolated committed pin
update is also valid. Engine reverse-certification is rolled back separately to a
previously certified consumer commit; it is never part of consumer rollback.

## Compatibility and integration evidence

A supported current consumer must stay green. `unavailable`, an unexpected
protocol rejection, a mixed provider commit, or a stale lockfile is an integration
regression, not an acceptable consequence of exact pinning.

An intentionally old protocol belongs in a separately named negative fixture.
Its expected `protocol.unsupportedVersion` result does not normalize failure in
the supported path, and the revision updater does not edit that fixture.

Consumer CI runs `engine-revision check` before compilation. An Engine-owned
reverse-certification check clones the exact public consumer commit, runs the
consumer's checker, verifies its manifest agrees with the Engine-owned
`engineCommit`, and then runs the real integration gate. Ordinary Engine
verification remains independent of sibling consumer checkouts.

Review evidence records values from the final committed state:

- the exact downstream commit under review;
- the provider commit read from that commit's `engine-source.json`;
- the focused and full gates run; and
- exact-SHA CI results where available.

If review fixes advance either repository, refresh the evidence from final
`HEAD`; do not reuse an earlier implementation SHA.

## Non-goals

This contract does not introduce:

- a package registry, release train, release manifest, or umbrella crate;
- a provider-owned list of consumers or their carrier files;
- a cross-repository update service or sibling-checkout fallback;
- separate language lanes without a demonstrated compatibility need;
- automatic protocol versioning; or
- a broad dependency-governance framework.

If three concrete consumers later expose stable repeated implementation code,
that evidence may justify a smaller shared helper. The command contract alone is
not permission to anticipate one.
