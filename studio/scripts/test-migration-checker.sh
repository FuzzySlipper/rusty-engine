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

sed 's/037acc81642d11df559bcb20a5d52cdba5b8d089/main/' \
  "$STUDIO_ROOT/demo-consumer-source.json" > "$TASK_TMP/floating-demo-source.json"
if STUDIO_DEMO_SOURCE="$TASK_TMP/floating-demo-source.json" \
  node "$STUDIO_ROOT/scripts/check-migration-plan.mjs" > "$TASK_TMP/output" 2>&1; then
  echo "Studio migration checker accepted a floating demo revision" >&2
  exit 1
fi
grep -q 'demo commit must be an exact Git revision' "$TASK_TMP/output"

echo "Studio migration checker negative probes passed"
