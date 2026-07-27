#!/usr/bin/env bash
set -euo pipefail

ENGINE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DONOR_ROOT="${1:-}"
DONOR_COMMIT="e4d6d1afb5b8387de4ff805d73b2041df29ee590"
OUTPUT_ROOT="$ENGINE_ROOT/migration/gameplay-mechanics-donor"

if [[ -z "$DONOR_ROOT" || ! -d "$DONOR_ROOT/.git" ]]; then
  echo "usage: $0 <asha-rpg-git-checkout>" >&2
  exit 2
fi

resolved_commit="$(git -C "$DONOR_ROOT" rev-parse "$DONOR_COMMIT^{commit}")"
if [[ "$resolved_commit" != "$DONOR_COMMIT" ]]; then
  echo "donor commit did not resolve exactly: $resolved_commit" >&2
  exit 1
fi

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
paths="$scratch/paths"
disposition="$scratch/disposition.tsv"
meta="$scratch/source.meta"

git -C "$DONOR_ROOT" ls-tree -r --name-only "$DONOR_COMMIT" > "$paths"
printf 'path\tdisposition\tsuccessor\tproof\tnotes\n' > "$disposition"

while IFS= read -r path; do
  disposition_kind="excluded"
  successor="none"
  proof="docs/migration/donor-provenance.md"
  notes="Repository topology or product-specific behavior was not transferred."

  case "$path" in
    README.md|docs/design.md|docs/first-wave-primitive-catalog.md)
      disposition_kind="adapted"
      successor="docs/design.md"
      proof="rust/crates/gameplay-mechanics/tests/contract.rs"
      notes="Common numeric and source semantics were rewritten behind Rusty components and named services."
      ;;
    docs/non-claims.md|governance/boundary-rules.md)
      disposition_kind="adopted"
      successor="docs/design.md"
      proof="scripts/audit-standalone.sh"
      notes="The useful negative boundary evidence is retained without donor governance topology."
      ;;
    docs/ruleweaver-core-composition.md|consumers/minimal-game/src/bin/ruleweaver_core.rs|examples/representative-actions.ts)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/examples/compositions.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm5.rs"
      notes="The d20-shaped composition became a downstream-owned direct-service example, not an Engine rules runtime."
      ;;
    crates/rpg-core/src/primitives.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/component.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm1.rs"
      notes="Bounded values and open identities became checked scalars, typed IDs, stats, and tracks."
      ;;
    crates/rpg-core/src/authority.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/effect.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm2.rs"
      notes="Source provenance, effect stacking, damage stages, and failure atomicity were rewritten without the authority aggregate."
      ;;
    crates/rpg-core/src/lib.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/lib.rs"
      proof="rust/crates/gameplay-mechanics/tests/contract.rs"
      notes="Selected public semantic families were re-exposed as Rusty-native components and services; the donor facade was not copied."
      ;;
    crates/rpg-compiler/src/compile.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/catalog.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm1.rs"
      notes="Strict reference admission and canonical ordering were retained in one bounded immutable catalog."
      ;;
    crates/rpg-compiler/src/diagnostic.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/error.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm5.rs"
      notes="Actionable typed rejection identity was retained without compiler-stage diagnostics."
      ;;
    crates/rpg-compiler/src/execute.rs)
      disposition_kind="adapted"
      successor="rust/crates/gameplay-mechanics/src/damage.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm3.rs"
      notes="Checked deterministic damage staging and late-failure rollback became one direct component-local service."
      ;;
    crates/rpg-compiler/tests/semantic_kernel.rs)
      disposition_kind="adopted"
      successor="rust/crates/gameplay-mechanics/tests/contract.rs"
      proof="rust/crates/gameplay-mechanics/tests/gm3.rs"
      notes="Determinism, unknown-reference, bounded decision, and late-failure cases were retained as rewritten provider proofs."
      ;;
    crates/rpg-core/Cargo.toml|crates/rpg-compiler/Cargo.toml|crates/rpg-ir/Cargo.toml|crates/rpg-runtime/Cargo.toml|crates/asha-rpg/Cargo.toml)
      notes="Historical crate topology and dependency grouping are deliberately excluded."
      ;;
    crates/rpg-compiler/src/registry.rs|crates/rpg-ir/src/*|crates/rpg-runtime/src/*|crates/asha-rpg/src/*)
      notes="Universal registry, IR, session, encounter, replay, and facade topology are deliberately excluded."
      ;;
    packages/authoring/src/*|packages/authoring/test/*|packages/authoring/dist/*|packages/ir/src/*|packages/ir/dist/*)
      notes="TypeScript authoring, generated vocabulary, package compiler, and cross-language IR authority are deliberately excluded."
      ;;
    examples/generate-*|scripts/generate-ir-vocabulary.mjs)
      notes="Generated-source and vocabulary-codegen paths are deliberately excluded."
      ;;
    governance/*|scripts/check-*|scripts/report-*|.github/*|package*.json|tsconfig*.json|Cargo.toml|Cargo.lock|AGENTS.md|.gitignore)
      notes="Donor repository governance, build, package, and CI infrastructure are not Engine runtime behavior."
      ;;
  esac

  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$path" "$disposition_kind" "$successor" "$proof" "$notes" >> "$disposition"
done < "$paths"

path_sha256="$(sha256sum "$paths" | awk '{print $1}')"
tree_sha="$(git -C "$DONOR_ROOT" rev-parse "$DONOR_COMMIT^{tree}")"
item_count="$(wc -l < "$paths" | tr -d ' ')"
{
  printf 'donor_repository\tFuzzySlipper/asha-rpg\n'
  printf 'donor_commit\t%s\n' "$DONOR_COMMIT"
  printf 'donor_tree\t%s\n' "$tree_sha"
  printf 'item_count\t%s\n' "$item_count"
  printf 'path_sha256\t%s\n' "$path_sha256"
} > "$meta"

mkdir -p "$OUTPUT_ROOT"
mv "$disposition" "$OUTPUT_ROOT/disposition.tsv"
mv "$meta" "$OUTPUT_ROOT/source.meta"
echo "wrote $item_count literal donor dispositions at $DONOR_COMMIT"
