#!/usr/bin/env bash
set -euo pipefail

# Build one source-independent, matched Engine development runtime for this
# machine: linux-x64, or win-x64 under Git Bash with the MSVC toolchain.
# This intentionally packages only Engine-owned host/browser/debug artifacts;
# product bundle layout and product staging remain outside this script.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "$(uname -s)" in
  MINGW* | MSYS*) TARGET=win-x64 EXE=.exe ;;
  *) TARGET=linux-x64 EXE= ;;
esac
OUTPUT="$REPO_ROOT/target/runtime-pack/$TARGET"

usage() {
  echo "usage: scripts/build-runtime-pack.sh [--output <new-directory>] [--desktop]" >&2
}

# --desktop builds the host with the desktop shell and ships Chromium's
# runtime in lib/cef. CEF's build downloads it into CEF_PATH (default
# target/cef); building it needs cmake and ninja.
DESKTOP=0

while (($#)); do
  case "$1" in
    --output)
      (($# >= 2)) || { usage; exit 2; }
      OUTPUT="$2"
      shift 2
      ;;
    --desktop)
      DESKTOP=1
      shift
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
# The browser shell and live-debug bundles, from the packages' fresh dist
# outputs so a clean checkout does not depend on stale local artifacts.
pnpm --dir render run build
HOST_FEATURES=()
if ((DESKTOP)); then
  export CEF_PATH="${CEF_PATH:-$REPO_ROOT/target/cef}"
  HOST_FEATURES=(--features desktop)
fi
cargo build --locked --release -p csharp-product-runtime "${HOST_FEATURES[@]}" \
  --bin rusty-product-host --bin rusty-live-debug --bin rusty-scene-render
cargo build --locked --release -p rusty-cli --bin rusty
# The Engine services, headless and in process, for products' unit tests
# (Rusty.Engine.Testing.EngineTestHost).
cargo build --locked --release -p csharp-engine-test-host --lib

install -d "$STAGE/bin" "$STAGE/share/browser/engine/live-debug-panel" \
  "$STAGE/share/live-debug-client" "$STAGE/share/live-debug-panel" "$STAGE/symbols"
release="${CARGO_TARGET_DIR:-target}/release"
for binary in rusty-product-host rusty-live-debug rusty-scene-render rusty; do
  install -m 755 "$release/$binary$EXE" "$STAGE/bin/$binary$EXE"
done
if [[ $TARGET == win-x64 ]]; then
  install -D -m 755 "$release/rusty_engine_test_host.dll" "$STAGE/lib/rusty_engine_test_host.dll"
else
  install -D -m 755 "$release/librusty_engine_test_host.so" "$STAGE/lib/librusty_engine_test_host.so"
fi
install -m 644 render/artifacts/product-browser-host/product-browser-host.js \
  "$STAGE/share/browser/engine/product-browser-host.js"
install -m 644 render/artifacts/live-debug-panel/index.js \
  "$STAGE/share/browser/engine/live-debug-panel/index.js"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/index.html \
  "$STAGE/share/browser/index.html"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/main.js \
  "$STAGE/share/browser/main.js"
find render/packages/live-debug-client/dist -maxdepth 1 -type f \
  ! -name '*.test.*' ! -name '*.tsbuildinfo' \
  -exec install -m 644 {} "$STAGE/share/live-debug-client/" \;
cp -a render/artifacts/live-debug-panel/. "$STAGE/share/live-debug-panel/"

if ((DESKTOP)); then
  # Only what Chromium loads at run time: the library (stripped of its
  # 1.2 GB of debug info), its resources, ICU data, V8 snapshot, ANGLE's GL
  # libraries, and one locale. SwiftShader (Chromium's software GPU) and the
  # bundled Vulkan loader are left out (#8860): the overlay imports Chromium's
  # frames as GPU textures, which needs the same system GPU driver and Vulkan
  # loader the world renderer uses, so neither is ever loaded.
  install -d "$STAGE/lib/cef/locales" "$STAGE/share/third-party/cef"
  CEF_RESOURCES=(chrome_100_percent.pak chrome_200_percent.pak resources.pak icudtl.dat v8_context_snapshot.bin)
  if [[ $TARGET == win-x64 ]]; then
    # Windows CEF carries no debug info in libcef.dll. chrome_elf is loaded
    # with it; ANGLE (libEGL, libGLESv2) draws through D3D11 with
    # d3dcompiler_47, and dxcompiler/dxil compile Dawn's D3D12 shaders.
    CEF_DIST="$(dirname "$(find "$CEF_PATH" -path '*cef_windows_x86_64/libcef.dll' -print -quit)")"
    [[ -f "$CEF_DIST/libcef.dll" ]] || { echo "CEF distribution not found under $CEF_PATH" >&2; exit 1; }
    for file in libcef.dll chrome_elf.dll libEGL.dll libGLESv2.dll d3dcompiler_47.dll dxcompiler.dll dxil.dll; do
      install -m 755 "$CEF_DIST/$file" "$STAGE/lib/cef/$file"
    done
  else
    CEF_DIST="$(dirname "$(find "$CEF_PATH" -path '*cef_linux_x86_64/libcef.so' -print -quit)")"
    [[ -f "$CEF_DIST/libcef.so" ]] || { echo "CEF distribution not found under $CEF_PATH" >&2; exit 1; }
    strip -o "$STAGE/lib/cef/libcef.so" "$CEF_DIST/libcef.so"
    CEF_RESOURCES+=(libEGL.so libGLESv2.so)
  fi
  for file in "${CEF_RESOURCES[@]}"; do
    install -m 644 "$CEF_DIST/$file" "$STAGE/lib/cef/$file"
  done
  [[ $TARGET == win-x64 ]] || chmod 755 "$STAGE/lib/cef/"*.so
  install -m 644 "$CEF_DIST/locales/en-US.pak" "$STAGE/lib/cef/locales/en-US.pak"
  install -m 644 "$CEF_DIST/CREDITS.html" "$STAGE/share/third-party/cef/CREDITS.html"
  # welding and grafting are MPL-2.0: their source ships with the binary, as
  # fidget-mesh's does.
  metadata=$(cargo metadata --format-version 1 --locked --all-features)
  for crate in welding grafting; do
    manifest=$(jq -r --arg name "$crate" '.packages[] | select(.name == $name) | .manifest_path' <<<"$metadata" | head -n 1 | tr -d '\r')
    [[ $TARGET != win-x64 ]] || manifest=$(cygpath -u "$manifest")
    [[ -f "$manifest" ]] || { echo "source of $crate not found" >&2; exit 1; }
    cp -a "$(dirname "$manifest")" "$STAGE/share/third-party/$crate"
  done
fi

# Include the corresponding source and license for our modified MPL component.
install -d "$STAGE/share/third-party"
cp -a rust/vendor/fidget-mesh "$STAGE/share/third-party/fidget-mesh"
# render-wgpu embeds DejaVu Sans; its license travels with the binaries.
install -D -m 0644 rust/crates/render-wgpu/fonts/LICENSE "$STAGE/share/third-party/dejavu-sans/LICENSE"

if [[ $TARGET == win-x64 ]]; then
  # MSVC writes symbols beside each binary as a .pdb.
  for binary in rusty-product-host rusty-live-debug rusty-scene-render rusty; do
    [[ ! -f "$release/${binary//-/_}.pdb" ]] || install -m 644 "$release/${binary//-/_}.pdb" "$STAGE/symbols/$binary.pdb"
  done
elif command -v objcopy >/dev/null 2>&1; then
  objcopy --only-keep-debug "$STAGE/bin/rusty-product-host" \
    "$STAGE/symbols/rusty-product-host.debug"
  objcopy --only-keep-debug "$STAGE/bin/rusty-live-debug" \
    "$STAGE/symbols/rusty-live-debug.debug"
  objcopy --only-keep-debug "$STAGE/bin/rusty-scene-render" \
    "$STAGE/symbols/rusty-scene-render.debug"
  objcopy --only-keep-debug "$STAGE/bin/rusty" \
    "$STAGE/symbols/rusty.debug"
  objcopy --only-keep-debug "$STAGE/lib/librusty_engine_test_host.so" \
    "$STAGE/symbols/librusty_engine_test_host.so.debug"
fi

IDENTITY="$("$STAGE/bin/rusty-product-host$EXE" --identity)"
REVISION="$(git rev-parse HEAD)"
# Source/compiler provenance travels with the symbols. Dirty contributor packs
# identify that fact explicitly; published packs should come from a clean commit.
{
  printf 'sourceRevision=%s\n' "$REVISION"
  if [[ -n "$(git status --porcelain)" ]]; then
    printf 'sourceDirty=true\n'
  else
    printf 'sourceDirty=false\n'
  fi
  printf 'profile=release\ndebug=%s\noptLevel=%s\n' \
    "${CARGO_PROFILE_RELEASE_DEBUG:-line-tables-only}" "${CARGO_PROFILE_RELEASE_OPT_LEVEL:-3}"
  printf 'rustflags=%s\n' "${RUSTFLAGS:-}"
  rustc --version --verbose
} > "$STAGE/symbols/build-info.txt"
{
  printf '{\n'
  printf '  "artifact": "rusty.product.runtime-pack",\n'
  printf '  "schemaVersion": 1,\n'
  printf '  "target": "%s",\n' "$TARGET"
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
