# Validation inventory

Start with the [operation audit and dispositions](validation-operation-audit.md)
for the actionable campaign; the raw rows below are a search index.

Build the task 7871 review queue from the repository root:

```sh
python3 scripts/inventory-validation.py
```

The command discovers files with `git ls-files --cached --others
--exclude-standard -z`. It scans first-party Rust, C#, TypeScript/JavaScript,
Python, shell, JSON, YAML, and TOML files; excludes dependency/vendor, build,
generated, lock, audit-output, and scanner-tool paths; and records counts in
the summary. Test and fixture records stay in the raw list with
`candidate_scope=test_or_fixture`.

Outputs are written under `artifacts/validation-audit-7871/`:

- `index.html` — standalone filterable browser view with source and copy links.
- `inventory.jsonl` — complete raw candidates; every row has
  `review_status=unreviewed`.
- `limits.tsv` — limit-only spreadsheet subset.
- `summary.json` and `summary.md` — scan scope, exclusions, and candidate counts.

The inventory is a text-pattern survey. It includes broad `if` and Rust
`match` candidates as a low-confidence fallback, so it does not establish
reachability, ownership, validity, or whether a limit is intrinsic or policy.
Keep human dispositions outside these raw generated files, since a rerun
replaces them.

## Reviewing the results

Start with production/config rows and the limit filter, then inspect the broader
validation rows. Counts describe candidate sites and categories overlap; a
workflow path containing `verify` can be a false positive. Open the source and
follow the owning operation before deciding. Generated code is excluded from the
scan; generator templates are included. Dependency-owned and dynamically
constructed checks require a separate pass. Text search cannot certify that all
semantic validation has been found.

Review related sites together when one fact travels through multiple layers,
but record a disposition for each site. A validator declaration, its call sites,
and a downstream repeat can have different answers. Suggested review fields:

| Field | Question |
|---|---|
| Failure prevented | What specific incorrect behavior occurs without this check? |
| Existing guarantee | Do the type, ABI, caller, or an earlier owning check already guarantee it? |
| Cost and frequency | Does it serialize, hash, allocate, copy or scan? Per item, call, frame, or load? |
| Valid behavior rejected | Which otherwise valid product operation does it forbid? |
| Ownership | Is this an Engine invariant, backend representation, caller-selected work budget, or product policy? |
| Disposition | Keep with reason; remove; consolidate at one owner; expose caller selection; or investigate. |

No name grants an exemption: `validate`, `safe`, `MAX`, a hash check, or even a
finite-number check needs a reason tied to the actual operation. Conversely,
removing a policy cap does not require removing index/length coherence or
lifetime handling. For an individual generated mesh, the byte budget is gone;
managed spans and native `u32` counts still have representation constraints.
Transport queues, complete-baseline budgets, packed resources, and recursion
work have their own checks awaiting review.

A productive first review order is expensive preflight work, duplicate
cross-layer checks, hardcoded capacities, restrictions on composition/lifecycle,
and remaining format/mathematical conditions. Keep decisions in a separate
ledger or the owning Den task, keyed by candidate ID and source context; source
edits can change IDs and line numbers. The raw snapshot remains unreviewed.

## First completed cleanup

Task **#7871** removed generated-mesh copied-byte/encoded-byte policy caps,
byte-derived vertex/index caps, throwaway JSON serialization solely to measure
size, and a duplicate full payload validation during admission. The final mesh
validation remains. A focused native test admits 2,900,000 vertices with more
than the former 64 MiB of copied position/normal streams and still rejects an
unrepresentable layout count and invalid indices. This is admission evidence,
not a claim of rendering arbitrarily large meshes. The separate 256-group cap
and delivery/resource policies remain inventory candidates.

## Starting review families

These source leads are unreviewed; even an apparent invariant still needs its
necessity and placement checked against the actual consumer.

| Family / starting source | Question to investigate |
|---|---|
| `rust/crates/voxel-asset/src/codec.rs`, `voxel-annotation/src/edit.rs` | Do hash construction and subsequent validation repeat canonical encoding and hashing? |
| `rust/crates/runtime-ui/src/model.rs`, `runtime-timeline/src/model.rs` | Which constructors serialize only to measure size, and which validators reconstruct or clone already admitted values? |
| `rust/crates/product-dev-host/src/model.rs`, `host.rs` | Which event, baseline, and fragment size scans reuse eventual wire bytes, and which encode again? |
| `rust/crates/asset-import/src/gltf_package.rs` | Can root parsing be shared with resource-closure discovery? |
| `rust/crates/content-store/src/batch.rs`, `write_set.rs` | Is prior-manifest validation repeated while computing identity? |
| `rust/crates/engine-spatial/src/occlusion.rs` | Does the public ray query repeat checks in its delegated implementation? |
| `csharp/Rusty.Engine.BindingGenerator/Program.cs` | What requires the generated 256 MiB owned-lease and 1,000,000-item caps beyond representation limits? |
| `render/packages/render-contracts/src/validation.ts` | Are descriptor and patch restrictions consistent, and are they needed by the backend? |
| `render/packages/renderer-three/src/particle-sink.ts` | Is the 4096-particle capacity a caller choice or an unnecessary fixed restriction? |

The browser view paginates rendering while filtering the full dataset.
`excluded-paths.jsonl` records each discovered path omitted from the scan.
Inline Rust test labeling is heuristic. Multiline expressions, custom admission
helpers, attributes/macros, generated expansions, and data-flow-dependent limits
need owner-by-owner inspection beyond this initial text survey.
