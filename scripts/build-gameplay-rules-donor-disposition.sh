#!/usr/bin/env bash
set -euo pipefail

ENGINE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DONOR_ROOT="${1:-}"
DONOR_COMMIT="e4d6d1afb5b8387de4ff805d73b2041df29ee590"
DONOR_TREE="3efee336ff8c6c9aeea2c37035d5258bfdf88847"
OUTPUT="$ENGINE_ROOT/migration/gameplay-rules-donor/disposition.tsv"

if [[ -z "$DONOR_ROOT" || ! -d "$DONOR_ROOT/.git" ]]; then
  echo "usage: $0 /absolute/path/to/asha-rpg" >&2
  exit 1
fi

actual_tree="$(git -C "$DONOR_ROOT" rev-parse "$DONOR_COMMIT^{tree}")"
if [[ "$actual_tree" != "$DONOR_TREE" ]]; then
  echo "unexpected asha-rpg donor tree: expected=$DONOR_TREE actual=$actual_tree" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUTPUT")"
{
  printf 'path\tdisposition\tsuccessor\tproof\tnotes\n'
  while IFS= read -r path; do
    disposition="excluded"
    successor="none"
    proof="docs/gameplay-rules-contract.md"
    notes="RPG semantics, runtime or product topology, generated output, and repository scaffolding are not part of the semantic-neutral support surface."

    case "$path" in
      crates/rpg-ir/src/play_bundle_artifact.rs)
        disposition="adapted"
        successor="rust/crates/gameplay-rules/src/package.rs"
        proof="fixtures/gameplay-rules/package-v1.canonical.json"
        notes="Only strict package identity, exact dependency, provenance, and envelope lessons are rewritten; the donor RPG schema is excluded."
        ;;
      crates/rpg-compiler/src/diagnostic.rs)
        disposition="adapted"
        successor="rust/crates/gameplay-rules/src/diagnostic.rs"
        proof="rust/crates/gameplay-rules/tests/contract.rs"
        notes="Bounded typed diagnostic shape and source correlation are rewritten without donor semantic codes or compiler ownership."
        ;;
      packages/authoring/src/canonical.ts)
        disposition="adapted"
        successor="rules/packages/gameplay-rules-authoring/src/canonical.ts"
        proof="rules/packages/gameplay-rules-authoring/src/authoring.test.ts"
        notes="Deterministic canonical JSON is retained as a cross-language lesson under the stricter integer-only schema-1 contract."
        ;;
      packages/authoring/src/play-bundle-compiler.ts)
        disposition="adapted"
        successor="rust/crates/gameplay-rules/src/resolve.rs"
        proof="rust/crates/gameplay-rules/tests/contract.rs"
        notes="Only deterministic exact package graph and provenance lessons are retained; semantic compilation and content patches stay downstream."
        ;;
      scripts/check-authoring-boundary.mjs)
        disposition="adapted"
        successor="rules/scripts/check-boundaries.mjs"
        proof="rules/scripts/check-boundaries.mjs"
        notes="The isolated authoring boundary-check pattern is retained without donor package topology or governance machinery."
        ;;
      scripts/generate-ir-vocabulary.mjs)
        disposition="adapted"
        successor="rust/crates/gameplay-rules/src/contract.rs"
        proof="rules/scripts/generate-contract.mjs"
        notes="Only the checked generated-contract drift pattern is retained; the donor RPG operation and capability vocabulary is excluded."
        ;;
      crates/rpg-compiler/src/artifact.rs)
        disposition="evidence"
        successor="Rust and TypeScript bounded canonical writers"
        proof="rust/crates/gameplay-rules/tests/contract.rs"
        notes="Artifact size, hash, and fail-closed decode behavior inform bounds; donor compiled RPG definitions are excluded."
        ;;
      crates/rpg-compiler/src/compile.rs)
        disposition="evidence"
        successor="rust/crates/gameplay-rules/src/package.rs"
        proof="rust/crates/gameplay-rules/tests/contract.rs"
        notes="Bounded validation and fail-before-publication behavior are evidence; all semantic compilation remains downstream."
        ;;
      crates/rpg-compiler/tests/semantic_kernel.rs)
        disposition="evidence"
        successor="rust/crates/gameplay-rules/tests/contract.rs"
        proof="rust/crates/gameplay-rules/tests/contract.rs"
        notes="Failure-path proof shape informs future fixtures without transferring the donor semantic kernel."
        ;;
      packages/authoring/src/normalize.ts)
        disposition="evidence"
        successor="rules/packages/gameplay-rules-contracts/src/validation.ts"
        proof="rules/packages/gameplay-rules-authoring/src/authoring.test.ts"
        notes="Executable-value rejection is retained as evidence; RPG semantic normalization stays downstream."
        ;;
      packages/authoring/test/authoring.test.ts|packages/authoring/test/contract-split.test.ts|packages/authoring/test/ruleset-packages.test.ts)
        disposition="evidence"
        successor="rules/packages/gameplay-rules-authoring/src/authoring.test.ts"
        proof="rules/packages/gameplay-rules-authoring/src/authoring.test.ts"
        notes="Immutable authoring, contract separation, and package rejection cases inform focused successor fixtures."
        ;;
      scripts/check-authoring-boundary.test.mjs)
        disposition="evidence"
        successor="rules/scripts/check-boundaries.mjs"
        proof="rules/scripts/check-boundaries.mjs"
        notes="Positive and negative boundary-check proof informs the successor gate."
        ;;
      docs/design.md|docs/non-claims.md|packages/authoring/README.md)
        disposition="evidence"
        successor="docs/gameplay-rules-contract.md"
        notes="Historical ownership and non-claim language is evidence only; the successor contract is authoritative."
        ;;
      crates/rpg-ir/*|packages/ir/*)
        notes="The universal formula, predicate, program, operation, reference, and generated RPG vocabulary are explicitly excluded."
        ;;
      crates/rpg-runtime/*|crates/rpg-core/src/authority.rs|crates/asha-rpg/*)
        notes="Authority session, encounter, lifecycle, replay, registry, and aggregate facade topology remain downstream or absent."
        ;;
      packages/authoring/dist/*)
        notes="Checked donor build output is not source and is not transferred."
        ;;
      packages/authoring/src/*|packages/authoring/test/*)
        notes="RPG-specific builders, schemas, content patches, catalogs, and semantic tests belong to Rusty D20 or another downstream domain."
        ;;
      consumers/*|examples/*)
        notes="Donor consumers, replay generators, and RPG examples are product evidence rather than Engine package support."
        ;;
      governance/*|.github/*|AGENTS.md|Cargo.lock|Cargo.toml|package-lock.json|package.json|tsconfig.*|.gitignore|README.md)
        notes="Donor repository governance, workspace, dependency, and build topology are not Engine runtime behavior."
        ;;
      crates/rpg-compiler/*)
        notes="Donor RPG compilation, execution, registry, CLI, and crate topology stay downstream; only separately named neutral lessons are retained."
        ;;
      crates/rpg-core/*)
        notes="RPG primitives and runtime authority belong to downstream semantic and component owners."
        ;;
      docs/*)
        notes="Historical RPG design, primitive catalog, encounter, and composition documents are evidence but not successor specification."
        ;;
      scripts/*)
        notes="Donor governance and change-amplification tooling is not part of the bounded support surface."
        ;;
    esac

    printf '%s\t%s\t%s\t%s\t%s\n' \
      "$path" "$disposition" "$successor" "$proof" "$notes"
  done < <(git -C "$DONOR_ROOT" ls-tree -r --name-only "$DONOR_COMMIT")
} > "$OUTPUT"

echo "wrote gameplay rules donor disposition: $OUTPUT"
