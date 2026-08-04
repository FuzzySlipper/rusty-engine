#!/usr/bin/env bash
set -euo pipefail

STUDIO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TASK_TMP="$(mktemp -d -t rusty-studio-migration-check.XXXXXX)"
trap 'rm -rf "$TASK_TMP"' EXIT

head -n -1 "$STUDIO_ROOT/donor-inventory.tsv" > "$TASK_TMP/missing-inventory.tsv"
if STUDIO_DONOR_INVENTORY="$TASK_TMP/missing-inventory.tsv" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a missing donor file" >&2
  exit 1
fi
grep -q 'expected 147 donor files' "$TASK_TMP/output"

awk 'NR != 2' "$STUDIO_ROOT/donor-surface-disposition.tsv" > "$TASK_TMP/missing-disposition.tsv"
if STUDIO_DONOR_DISPOSITION="$TASK_TMP/missing-disposition.tsv" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted an undisposed donor surface" >&2
  exit 1
fi
grep -q 'has 0 surface dispositions' "$TASK_TMP/output"

awk -F '\t' '$1 != "voxel-annotation"' "$STUDIO_ROOT/owner-adoption.tsv" > "$TASK_TMP/missing-owner.tsv"
if STUDIO_OWNER_ADOPTION="$TASK_TMP/missing-owner.tsv" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a missing Engine owner" >&2
  exit 1
fi
grep -q 'current Rust workspace owner lacks Studio classification: voxel-annotation' "$TASK_TMP/output"

sed -E 's/"commit": "[0-9a-f]{40}"/"commit": "main"/' \
  "$STUDIO_ROOT/demo-consumer-source.json" > "$TASK_TMP/floating-demo-source.json"
if STUDIO_DEMO_SOURCE="$TASK_TMP/floating-demo-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a floating demo revision" >&2
  exit 1
fi
grep -q 'demo commit must be an exact Git revision' "$TASK_TMP/output"

sed -E 's/"engineCommit": "[0-9a-f]{40}"/"engineCommit": "main"/' \
  "$STUDIO_ROOT/demo-consumer-source.json" > "$TASK_TMP/floating-demo-engine-source.json"
if STUDIO_DEMO_SOURCE="$TASK_TMP/floating-demo-engine-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a floating demo Engine revision" >&2
  exit 1
fi
grep -q 'demo engineCommit must be an exact Git revision' "$TASK_TMP/output"

sed -E 's/"engineCommit": "[0-9a-f]{40}"/"engineCommit": "main"/' \
  "$STUDIO_ROOT/voxel-consumer-source.json" > "$TASK_TMP/floating-voxel-source.json"
if STUDIO_VOXEL_SOURCE="$TASK_TMP/floating-voxel-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a floating voxel Engine revision" >&2
  exit 1
fi
grep -q 'engineCommit must be an exact Git revision' "$TASK_TMP/output"

sed -E 's/"engineCommit": "[0-9a-f]{40}"/"engineCommit": "refs\/heads\/main"/' \
  "$STUDIO_ROOT/demo-consumer-source.json" > "$TASK_TMP/rolling-demo-engine-source.json"
if ! STUDIO_DEMO_SOURCE="$TASK_TMP/rolling-demo-engine-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" --mode=development > "$TASK_TMP/output" 2>&1; then
  cat "$TASK_TMP/output" >&2
  echo "Studio migration checker rejected the rolling development Engine ref" >&2
  exit 1
fi
grep -q 'Studio migration plan passed in development mode' "$TASK_TMP/output"

sed -E 's/"evidenceEngineCommit": "[0-9a-f]{40}"/"evidenceEngineCommit": "main"/' \
  "$STUDIO_ROOT/voxel-consumer-source.json" > "$TASK_TMP/floating-voxel-evidence-source.json"
if STUDIO_VOXEL_SOURCE="$TASK_TMP/floating-voxel-evidence-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a floating voxel evidence revision" >&2
  exit 1
fi
grep -q 'evidenceEngineCommit must be an exact Git revision' "$TASK_TMP/output"

sed "s#readFileSync('studio/demo-consumer-source.json'#readFileSync('studio/floating-source.json'#" \
  "$STUDIO_ROOT/../.github/workflows/studio-demo-integration.yml" \
  > "$TASK_TMP/floating-workflow.yml"
if STUDIO_INTEGRATION_WORKFLOW="$TASK_TMP/floating-workflow.yml" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a workflow disconnected from the reviewed pin" >&2
  exit 1
fi
grep -q 'Studio integration workflow does not use the declared demo pin' "$TASK_TMP/output"

sed 's#./scripts/engine-revision check#./scripts/engine-revision update#' \
  "$STUDIO_ROOT/../scripts/verify-studio-demo-integration.sh" \
  > "$TASK_TMP/integration-without-consumer-check.sh"
if STUDIO_DEMO_INTEGRATION_SCRIPT="$TASK_TMP/integration-without-consumer-check.sh" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted integration without the consumer revision check" >&2
  exit 1
fi
grep -q 'Studio demo integration omits consumer revision proof' "$TASK_TMP/output"

sed "s#readFileSync('studio/voxel-consumer-source.json'#readFileSync('studio/floating-voxel-source.json'#" \
  "$STUDIO_ROOT/../.github/workflows/studio-voxel-integration.yml" \
  > "$TASK_TMP/floating-voxel-workflow.yml"
if STUDIO_VOXEL_INTEGRATION_WORKFLOW="$TASK_TMP/floating-voxel-workflow.yml" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a voxel workflow disconnected from its reviewed pin" >&2
  exit 1
fi
grep -q 'Studio voxel workflow does not use the declared consumer pin' "$TASK_TMP/output"

sed 's#./scripts/engine-revision check#./scripts/engine-revision update#' \
  "$STUDIO_ROOT/../scripts/verify-studio-voxel-integration.sh" \
  > "$TASK_TMP/voxel-integration-without-consumer-check.sh"
if STUDIO_VOXEL_INTEGRATION_SCRIPT="$TASK_TMP/voxel-integration-without-consumer-check.sh" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted voxel integration without revision check" >&2
  exit 1
fi
grep -q 'Studio voxel integration omits consumer revision proof' "$TASK_TMP/output"

echo "Studio migration checker negative probes passed"
