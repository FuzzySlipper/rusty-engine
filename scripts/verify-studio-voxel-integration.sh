#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VOXEL_ROOT="${1:-${RUSTY_ENGINE_VOXEL_CONSUMER_ROOT:-}}"
PIN_FILE="$REPO_ROOT/studio/voxel-consumer-source.json"

if [[ -z "$VOXEL_ROOT" ]]; then
  echo "usage: $0 <absolute-rusty-engine-voxels-root>" >&2
  exit 2
fi
if [[ "$VOXEL_ROOT" != /* ]]; then
  echo "rusty-engine-voxels root must be absolute: $VOXEL_ROOT" >&2
  exit 2
fi
if [[ ! -f "$VOXEL_ROOT/Cargo.toml" || ! -f "$VOXEL_ROOT/engine-source.json" ]]; then
  echo "not a rusty-engine-voxels checkout: $VOXEL_ROOT" >&2
  exit 1
fi

PIN_OUTPUT="$(node --input-type=module - "$PIN_FILE" <<'NODE'
import { readFileSync } from 'node:fs';

const pin = JSON.parse(readFileSync(process.argv[2], 'utf8'));
if (
  pin.schemaVersion !== 1
  || pin.repository !== 'FuzzySlipper/rusty-engine-voxels'
  || pin.publicRepository !== 'https://github.com/FuzzySlipper/rusty-engine-voxels'
) {
  throw new Error('voxel consumer pin has an unsupported repository identity');
}
for (const field of ['commit', 'engineCommit', 'evidenceEngineCommit']) {
  if (typeof pin[field] !== 'string' || !/^[0-9a-f]{40}$/.test(pin[field])) {
    throw new Error(`voxel consumer pin ${field} must be one exact 40-character commit`);
  }
}
if (
  pin.projectFile !== 'content/projects/voxel-lab.project.json'
  || pin.largeProjectFile !== 'content/projects/retro-character-high-fidelity.project.json'
  || pin.runtimeReport !== 'evidence/initial-animated-voxel-report.json'
  || pin.qualityReport !== 'evidence/high-fidelity-animated-voxel-report.json'
  || pin.dataPlaneReport !== 'evidence/mesh-data-plane.json'
  || pin.cargoPackage !== 'rusty-engine-voxels'
  || pin.adapterBinary !== 'rusty-engine-voxels-studio-adapter'
) {
  throw new Error('voxel consumer pin has an unsupported integration target');
}
process.stdout.write([
  pin.commit,
  pin.engineCommit,
  pin.evidenceEngineCommit,
  pin.projectFile,
  pin.largeProjectFile,
  pin.runtimeReport,
  pin.qualityReport,
  pin.dataPlaneReport,
  pin.adapterBinary,
].join('\n'));
NODE
)"
mapfile -t PIN_VALUES <<< "$PIN_OUTPUT"
EXPECTED_COMMIT="${PIN_VALUES[0]:-}"
EXPECTED_ENGINE_COMMIT="${PIN_VALUES[1]:-}"
EXPECTED_EVIDENCE_ENGINE_COMMIT="${PIN_VALUES[2]:-}"
PROJECT_FILE="${PIN_VALUES[3]:-}"
LARGE_PROJECT_FILE="${PIN_VALUES[4]:-}"
RUNTIME_REPORT="${PIN_VALUES[5]:-}"
QUALITY_REPORT="${PIN_VALUES[6]:-}"
DATA_PLANE_REPORT="${PIN_VALUES[7]:-}"
ADAPTER_BINARY="${PIN_VALUES[8]:-}"

VOXEL_ROOT="$(realpath "$VOXEL_ROOT")"
VOXEL_TOP="$(git -C "$VOXEL_ROOT" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ "$VOXEL_TOP" != "$VOXEL_ROOT" ]]; then
  echo "rusty-engine-voxels root must be an explicit checkout root: $VOXEL_ROOT" >&2
  exit 1
fi
VOXEL_COMMIT="$(git -C "$VOXEL_ROOT" rev-parse HEAD)"
if [[ "$VOXEL_COMMIT" != "$EXPECTED_COMMIT" ]]; then
  echo "rusty-engine-voxels revision mismatch: expected $EXPECTED_COMMIT, found $VOXEL_COMMIT" >&2
  exit 1
fi
VOXEL_STATUS="$(git -C "$VOXEL_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$VOXEL_STATUS" ]]; then
  echo "rusty-engine-voxels checkout must be clean, including non-ignored untracked inputs:" >&2
  echo "$VOXEL_STATUS" >&2
  exit 1
fi

if [[ ! -x "$VOXEL_ROOT/scripts/engine-revision" ]]; then
  echo "reviewed voxel consumer does not provide ./scripts/engine-revision" >&2
  exit 1
fi
REVISION_CHECK="$(cd "$VOXEL_ROOT" && ./scripts/engine-revision check)"
echo "$REVISION_CHECK"
if ! grep -Fq \
  "Engine revision $EXPECTED_ENGINE_COMMIT is coherent across" \
  <<< "$REVISION_CHECK"; then
  echo "consumer revision check did not certify the Engine reverse pin" >&2
  exit 1
fi

node --input-type=module - \
  "$VOXEL_ROOT/engine-source.json" \
  "$VOXEL_ROOT/$RUNTIME_REPORT" \
  "$VOXEL_ROOT/$QUALITY_REPORT" \
  "$VOXEL_ROOT/$DATA_PLANE_REPORT" \
  "$EXPECTED_ENGINE_COMMIT" \
  "$EXPECTED_EVIDENCE_ENGINE_COMMIT" <<'NODE'
import { readFileSync } from 'node:fs';

const source = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const runtimeReport = JSON.parse(readFileSync(process.argv[3], 'utf8'));
const qualityReport = JSON.parse(readFileSync(process.argv[4], 'utf8'));
const dataPlaneReport = JSON.parse(readFileSync(process.argv[5], 'utf8'));
const expectedEngineCommit = process.argv[6];
const expectedEvidenceEngineCommit = process.argv[7];
if (
  source.schemaVersion !== 1
  || source.repository !== 'https://github.com/FuzzySlipper/rusty-engine'
  || source.commit !== expectedEngineCommit
  || source.studioDirectory !== 'studio'
) {
  throw new Error('consumer does not use the reviewed exact public Engine revision');
}
if (
  runtimeReport.runtime?.engineRevision !== expectedEvidenceEngineCommit
  || qualityReport.runtime?.engineRevision !== expectedEvidenceEngineCommit
  || dataPlaneReport.engineRevision !== expectedEvidenceEngineCommit
) {
  throw new Error('consumer reports drifted from their recorded historical Engine revision');
}
if (
  dataPlaneReport.before?.completeProjectionJsonBytes !== 54_564_714
  || dataPlaneReport.after?.studioControlResponseBytes !== 24_805
  || dataPlaneReport.after?.packedResourceBytes !== 34_541_056
  || !Number.isFinite(dataPlaneReport.after?.nodeJsonParseMilliseconds)
  || !Number.isFinite(dataPlaneReport.after?.chromiumJsonParseMilliseconds)
) {
  throw new Error('consumer mesh data-plane evidence is incomplete or drifted');
}
const behavior = runtimeReport.runtime?.behavior;
for (const field of [
  'onceEnded',
  'repeatWrappedToFirstFrame',
  'pausedFrameStayedStable',
  'resumedToLaterFrame',
  'postureRoundTripMatched',
  'projectReopenMatched',
  'missingAssetRejected',
  'corruptAssetRejected',
  'collisionStayedStableDuringPlayback',
  'durableProjectBytesUnchanged',
  'durableObjectBytesUnchanged',
]) {
  if (behavior?.[field] !== true) throw new Error(`runtime behavior evidence is missing ${field}`);
}
if (behavior?.savedFrame !== 'default' || behavior?.collisionKind !== 'stableFrame') {
  throw new Error('consumer must save and collide against the canonical default pose');
}
for (const field of [
  'canonicalObjectBytes',
  'resolvedCellBytes',
  'uniqueMeshPayloadBytes',
  'admissionAndMeshingMicroseconds',
]) {
  if (
    !Number.isSafeInteger(qualityReport.runtime?.resources?.[field])
    || qualityReport.runtime.resources[field] <= 0
  ) {
    throw new Error(`runtime resource measurement is missing ${field}`);
  }
}
if (qualityReport.runtime?.frameSwitch?.emittedFrameSwaps !== 512) {
  throw new Error('frame-switch measurement does not contain the checked 512 swaps');
}
const clips = new Map((qualityReport.quality?.clips ?? []).map((clip) => [clip.clipId, clip]));
for (const clipId of ['clip/idle', 'clip/run', 'clip/jump']) {
  const clip = clips.get(clipId);
  if (clip === undefined || clip.frames.length < 2 || clip.paletteStable !== true) {
    throw new Error(`quality evidence is incomplete for ${clipId}`);
  }
  if (Math.min(...clip.frames.map((frame) => frame.sourceVoxelSilhouetteJaccard)) < 0.9) {
    throw new Error(`high-fidelity silhouette evidence regressed below 0.9 for ${clipId}`);
  }
}
console.log(JSON.stringify({
  kind: 'voxelConsumerQualityEvidence',
  engineRevision: expectedEngineCommit,
  evidenceEngineRevision: expectedEvidenceEngineCommit,
  clips: [...clips.keys()],
  canonicalObjectBytes: qualityReport.runtime.resources.canonicalObjectBytes,
  uniqueMeshPayloadBytes: qualityReport.runtime.resources.uniqueMeshPayloadBytes,
  admissionAndMeshingMicroseconds:
    qualityReport.runtime.resources.admissionAndMeshingMicroseconds,
  averageProjectionCpuNanosecondsPerSwap:
    qualityReport.runtime.frameSwitch.averageProjectionCpuNanosecondsPerSwap,
  studioControlResponseBytes: dataPlaneReport.after.studioControlResponseBytes,
  packedResourceBytes: dataPlaneReport.after.packedResourceBytes,
  browserJsonParseMilliseconds: dataPlaneReport.after.chromiumJsonParseMilliseconds,
}));
NODE

cd "$VOXEL_ROOT"
./scripts/verify.sh
STUDIO_SMOKE_OUTPUT="$(mktemp /tmp/rusty-engine-voxel-studio-smoke.XXXXXX)"
STUDIO_TEST_ROOT="$(mktemp -d /tmp/rusty-engine-voxel-browser.XXXXXX)"
STUDIO_SETTINGS_ROOT="$(mktemp -d /tmp/rusty-engine-voxel-settings.XXXXXX)"
cleanup() {
  rm -f -- "$STUDIO_SMOKE_OUTPUT"
  rm -rf -- "$STUDIO_TEST_ROOT"
  rm -rf -- "$STUDIO_SETTINGS_ROOT"
}
trap cleanup EXIT
./scripts/verify-studio.sh | tee "$STUDIO_SMOKE_OUTPUT"
if ! grep -Fq '"missingAssetRejected":true' "$STUDIO_SMOKE_OUTPUT" \
  || ! grep -Fq '"corruptAssetRejected":true' "$STUDIO_SMOKE_OUTPUT"; then
  echo "consumer Studio smoke did not prove missing and corrupt object rejection" >&2
  exit 1
fi
EXPECTED_LARGE_RESOURCE_BYTES="$(node --input-type=module - "$STUDIO_SMOKE_OUTPUT" <<'NODE'
import { readFileSync } from 'node:fs';

const records = readFileSync(process.argv[2], 'utf8')
  .split('\n')
  .filter((line) => line.startsWith('{'))
  .map((line) => JSON.parse(line));
const evidence = records.find((record) => record.protocolVersion === 12);
if (
  evidence === undefined
  || !Number.isSafeInteger(evidence.highFidelityPackedResourceBytes)
  || evidence.highFidelityPackedResourceBytes <= 0
) {
  throw new Error('consumer Studio smoke omitted the current high-fidelity resource size');
}
process.stdout.write(String(evidence.highFidelityPackedResourceBytes));
NODE
)"

cp -a "$VOXEL_ROOT/content" "$STUDIO_TEST_ROOT/content"
cp -a "$VOXEL_ROOT/evidence" "$STUDIO_TEST_ROOT/evidence"

cd "$REPO_ROOT"
pnpm --dir studio run build
cargo build --locked --manifest-path "$VOXEL_ROOT/Cargo.toml" --bin "$ADAPTER_BINARY"

RUSTY_STUDIO_ADAPTER_BINARY="$VOXEL_ROOT/target/debug/$ADAPTER_BINARY" \
RUSTY_STUDIO_PROJECT_ROOT="$STUDIO_TEST_ROOT" \
RUSTY_STUDIO_PROJECT_FILE="$PROJECT_FILE" \
RUSTY_STUDIO_LARGE_PROJECT_FILE="$LARGE_PROJECT_FILE" \
RUSTY_STUDIO_RUNTIME_REPORT="$RUNTIME_REPORT" \
RUSTY_STUDIO_SETTINGS_ROOT="$STUDIO_SETTINGS_ROOT" \
RUSTY_STUDIO_EXPECTED_LARGE_RESOURCE_BYTES="$EXPECTED_LARGE_RESOURCE_BYTES" \
RUSTY_STUDIO_ENGINE_COMMIT="$EXPECTED_ENGINE_COMMIT" \
pnpm --dir studio exec playwright test \
  --config voxel-consumer.playwright.config.ts

VOXEL_STATUS="$(git -C "$VOXEL_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$VOXEL_STATUS" ]]; then
  echo "integration verification changed the reviewed voxel consumer checkout:" >&2
  echo "$VOXEL_STATUS" >&2
  exit 1
fi
