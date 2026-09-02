#!/usr/bin/env bash
set -euo pipefail

# Build one source-independent, matched Linux-x64 Engine development runtime.
# This intentionally packages only Engine-owned host/browser/debug artifacts;
# product bundle layout and product staging remain outside this script.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="$REPO_ROOT/target/runtime-pack/linux-x64"

usage() {
  echo "usage: scripts/build-runtime-pack.sh [--output <new-directory>]" >&2
}

while (($#)); do
  case "$1" in
    --output)
      (($# >= 2)) || { usage; exit 2; }
      OUTPUT="$2"
      shift 2
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

case "$OUTPUT" in
  /*) ;;
  *) OUTPUT="$REPO_ROOT/$OUTPUT" ;;
esac

if [[ -e "$OUTPUT" ]]; then
  echo "runtime-pack output already exists: $OUTPUT" >&2
  echo "choose a new --output directory; this builder never replaces an artifact" >&2
  exit 2
fi

STAGE_PARENT="$(dirname "$OUTPUT")"
mkdir -p "$STAGE_PARENT"
STAGE="$(mktemp -d "$STAGE_PARENT/.runtime-pack.XXXXXX")"
cleanup() { rm -rf -- "$STAGE"; }
trap cleanup EXIT

cd "$REPO_ROOT"
pnpm --dir render run bundle:application-host-artifact
pnpm --dir render run bundle:product-browser-host-artifact
pnpm --dir render --filter @rusty-engine/live-debug-client run build
pnpm --dir studio run build:live-debug-panel-artifact
cargo build --locked --release -p csharp-product-runtime \
  --bin rusty-product-host --bin rusty-live-debug
cargo build --locked --release -p rusty-cli --bin rusty

install -d "$STAGE/bin" "$STAGE/share/browser/engine/live-debug-panel" \
  "$STAGE/share/live-debug-client" "$STAGE/share/live-debug-panel" "$STAGE/symbols"
install -m 755 target/release/rusty-product-host "$STAGE/bin/rusty-product-host"
install -m 755 target/release/rusty-live-debug "$STAGE/bin/rusty-live-debug"
install -m 755 target/release/rusty "$STAGE/bin/rusty"
install -m 644 render/artifacts/product-browser-host/product-browser-host.js \
  "$STAGE/share/browser/engine/product-browser-host.js"
install -m 644 studio/artifacts/live-debug-panel/index.js \
  "$STAGE/share/browser/engine/live-debug-panel/index.js"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/index.html \
  "$STAGE/share/browser/index.html"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/main.js \
  "$STAGE/share/browser/main.js"
find render/packages/live-debug-client/dist -maxdepth 1 -type f \
  ! -name '*.test.*' ! -name '*.tsbuildinfo' \
  -exec install -m 644 {} "$STAGE/share/live-debug-client/" \;
cp -a studio/artifacts/live-debug-panel/. "$STAGE/share/live-debug-panel/"

if command -v objcopy >/dev/null 2>&1; then
  objcopy --only-keep-debug "$STAGE/bin/rusty-product-host" \
    "$STAGE/symbols/rusty-product-host.debug"
  objcopy --only-keep-debug "$STAGE/bin/rusty-live-debug" \
    "$STAGE/symbols/rusty-live-debug.debug"
  objcopy --only-keep-debug "$STAGE/bin/rusty" \
    "$STAGE/symbols/rusty.debug"
fi

IDENTITY="$($STAGE/bin/rusty-product-host --identity)"
REVISION="$(git rev-parse HEAD)"
{
  printf '{\n'
  printf '  "artifact": "rusty.product.runtime-pack",\n'
  printf '  "schemaVersion": 1,\n'
  printf '  "target": "linux-x64",\n'
  printf '  "sourceRevision": "%s",\n' "$REVISION"
  printf '  "runtime": %s,\n' "$IDENTITY"
  printf '  "files": [\n'
  first=1
  while IFS= read -r file; do
    relative="${file#"$STAGE/"}"
    digest="$(sha256sum "$file" | awk '{print $1}')"
    size="$(wc -c < "$file" | tr -d '[:space:]')"
    if ((first)); then first=0; else printf ',\n'; fi
    printf '    {"path":"%s","sha256":"%s","bytes":%s}' "$relative" "$digest" "$size"
  done < <(find "$STAGE" -type f ! -name runtime-manifest.json -print | LC_ALL=C sort)
  printf '\n  ]\n}\n'
} > "$STAGE/runtime-manifest.json"

mv -- "$STAGE" "$OUTPUT"
trap - EXIT
printf 'runtime pack built: %s\n' "$OUTPUT"
