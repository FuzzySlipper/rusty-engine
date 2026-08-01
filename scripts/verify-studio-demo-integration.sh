#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"
MODE="${2:-all}"
PIN_FILE="$REPO_ROOT/studio/demo-consumer-source.json"

if [[ -z "$DEMO_ROOT" ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root> [all|browser|entity-inspector]" >&2
  exit 2
fi
if [[ "$DEMO_ROOT" != /* ]]; then
  echo "rusty-engine-demo root must be absolute: $DEMO_ROOT" >&2
  exit 2
fi
if [[
  ! -f "$DEMO_ROOT/Cargo.toml"
  || ! -f "$DEMO_ROOT/AGENTS.md"
  || ! -f "$DEMO_ROOT/engine-source.json"
  || ! -x "$DEMO_ROOT/scripts/engine-revision"
]]; then
  echo "not a rusty-engine-demo checkout: $DEMO_ROOT" >&2
  exit 1
fi
case "$MODE" in
  all)
    RUN_BROWSER=1
    RUN_ENTITY_INSPECTOR=1
    ;;
  browser)
    RUN_BROWSER=1
    RUN_ENTITY_INSPECTOR=0
    ;;
  entity-inspector)
    RUN_BROWSER=0
    RUN_ENTITY_INSPECTOR=1
    ;;
  *)
    echo "unsupported integration mode: $MODE" >&2
    echo "usage: $0 <absolute-rusty-engine-demo-root> [all|browser|entity-inspector]" >&2
    exit 2
    ;;
esac

REVISION_OUTPUT="$(node "$REPO_ROOT/studio/scripts/check-demo-consumer-revision.mjs" \
  "$PIN_FILE" \
  "$DEMO_ROOT/engine-source.json" \
  --shell-values)"
mapfile -t REVISION_VALUES <<< "$REVISION_OUTPUT"
EXPECTED_REPOSITORY="${REVISION_VALUES[0]:-}"
EXPECTED_COMMIT="${REVISION_VALUES[1]:-}"
EXPECTED_ENGINE_COMMIT="${REVISION_VALUES[2]:-}"
REVISION_EVIDENCE="${REVISION_VALUES[3]:-}"
EXPECTED_PUBLIC_REPOSITORY="$(node -p "require('$PIN_FILE').publicRepository")"
EXPECTED_ADAPTER_ID="$(node -p "require('$PIN_FILE').adapterId")"
ENGINE_SOURCE_COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD)"
DEMO_ROOT="$(realpath "$DEMO_ROOT")"
DEMO_TOP="$(git -C "$DEMO_ROOT" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ "$DEMO_TOP" != "$DEMO_ROOT" ]]; then
  echo "rusty-engine-demo root must be an explicit checkout root: $DEMO_ROOT" >&2
  exit 1
fi
DEMO_COMMIT="$(git -C "$DEMO_ROOT" rev-parse HEAD)"
if [[ "$DEMO_COMMIT" != "$EXPECTED_COMMIT" ]]; then
  echo "rusty-engine-demo revision mismatch: expected $EXPECTED_REPOSITORY@$EXPECTED_COMMIT, found $DEMO_COMMIT" >&2
  exit 1
fi
DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "rusty-engine-demo checkout must be clean, including non-ignored untracked inputs:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi

echo "$REVISION_EVIDENCE"
(cd "$DEMO_ROOT" && ./scripts/engine-revision check)
DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "the consumer Engine revision check changed the reviewed checkout:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi

cd "$REPO_ROOT"
if [[ "$RUN_BROWSER" == 1 ]]; then
  pnpm --dir studio run check:boundaries
  pnpm --dir studio run build
fi
cargo build --locked --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter
if [[ "$RUN_BROWSER" == 1 ]]; then
  node studio/test/integration/demo-adapter.mjs \
    --demo-root "$DEMO_ROOT" \
    --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
  node studio/test/integration/demo-voxel-objects.mjs \
    --demo-root "$DEMO_ROOT" \
    --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
  RUSTY_STUDIO_ENGINE_SOURCE_COMMIT="$ENGINE_SOURCE_COMMIT" \
  RUSTY_STUDIO_CONSUMER_REPOSITORY="$EXPECTED_PUBLIC_REPOSITORY" \
  RUSTY_STUDIO_CONSUMER_COMMIT="$EXPECTED_COMMIT" \
  RUSTY_STUDIO_ADAPTER_BUILD_COMMIT="$EXPECTED_COMMIT" \
  RUSTY_STUDIO_EXPECTED_ADAPTER_ID="$EXPECTED_ADAPTER_ID" \
    ./scripts/verify-studio-browser-integration.sh "$DEMO_ROOT"
fi
if [[ "$RUN_ENTITY_INSPECTOR" == 1 ]]; then
  ./scripts/verify-studio-entity-inspector-integration.sh "$DEMO_ROOT"
fi

DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "integration verification changed the reviewed consumer checkout:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi

node --input-type=module - \
  "$EXPECTED_REPOSITORY" \
  "$EXPECTED_COMMIT" \
  "$EXPECTED_ENGINE_COMMIT" \
  "$MODE" <<'NODE'
const [consumerRepository, consumerCommit, engineCommit, mode] = process.argv.slice(2);
console.log(JSON.stringify({
  kind: 'studioDemoIntegrationCertified',
  consumerRepository,
  consumerCommit,
  engineRepository: 'FuzzySlipper/rusty-engine',
  engineCommit,
  mode,
}));
NODE
