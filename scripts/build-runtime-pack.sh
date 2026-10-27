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
  echo "usage: scripts/build-runtime-pack.sh [--output <new-directory>] [--desktop] [--dlss <dlss-sdk>]" >&2
}

timed() {
  local label=$1 started=$SECONDS
  shift
  "$@"
  printf 'runtime-pack %s: %ss\n' "$label" "$((SECONDS - started))" >&2
}

# --desktop builds the host with the desktop shell and both of the window's
# UI engines: Chromium, whose runtime ships in lib/cef, and Servo
# (renderer.windowUi = servo). CEF's build downloads Chromium into CEF_PATH
# (default target/cef) and needs cmake and ninja; Servo's needs Python 3.
DESKTOP=0
# --dlss builds the renderer with NVIDIA DLSS from the DLSS SDK checkout
# given, and ships its library beside the host with NVIDIA's licence text
# (docs/lighting-and-sky.md#dlss). The build also needs clang and the Vulkan
# headers under VULKAN_SDK (default /usr on Linux).
DLSS_SDK_DIR=

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
    --dlss)
      (($# >= 2)) || { usage; exit 2; }
      DLSS_SDK_DIR="$(cd "$2" && pwd)"
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
# The browser shell and live-debug bundles, from the packages' fresh dist
# outputs so a clean checkout does not depend on stale local artifacts.
timed browser-build pnpm --dir render run build
HOST_FEATURES=()
SCENE_FEATURES=()
if ((DESKTOP)); then
  export CEF_PATH="${CEF_PATH:-$REPO_ROOT/target/cef}"
  HOST_FEATURES+=(servo)
fi
if [[ -n $DLSS_SDK_DIR ]]; then
  # dlss_wgpu's build reads both; nothing reads them at run time.
  export DLSS_SDK="$DLSS_SDK_DIR"
  export VULKAN_SDK="${VULKAN_SDK:-/usr}"
  if [[ $TARGET == win-x64 ]]; then
    # The build script is a Windows program: give it Windows paths.
    DLSS_SDK=$(cygpath -m "$DLSS_SDK")
    VULKAN_SDK=$(cygpath -m "$VULKAN_SDK")
    # NGX's dynamic-CRT library clashes with nethost's static one
    # (LNK2038); with a static C runtime dlss_wgpu links NGX's static-CRT
    # library. With Servo, whose SpiderMonkey needs the dynamic C runtime,
    # nethost links as a DLL instead and NGX's dynamic-CRT library fits.
    if ((!DESKTOP)); then
      export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-feature=+crt-static"
    fi
  fi
  HOST_FEATURES+=(dlss)
  SCENE_FEATURES+=(dlss)
fi
if ((${#HOST_FEATURES[@]})); then
  HOST_FEATURES=(--features "$(IFS=,; echo "${HOST_FEATURES[*]}")")
fi
# On Windows the desktop pack ships nethost.dll; the renderer links it too, so
# with DLSS (built without crt-static there) nethost's static library does not
# clash with NGX's C runtime.
if [[ $TARGET == win-x64 ]] && ((DESKTOP)); then
  SCENE_FEATURES+=(nethost-sys/dll)
fi
if ((${#SCENE_FEATURES[@]})); then
  SCENE_FEATURES=(--features "$(IFS=,; echo "${SCENE_FEATURES[*]}")")
fi
timed host-build cargo build --locked --release -p csharp-product-runtime "${HOST_FEATURES[@]}" \
  --bin rusty-product-host --bin rusty-live-debug
# The offline renderer opens no window: built without the desktop shell and
# its UI engines (cargo features apply to the whole package), it is about
# 380 MB smaller.
timed offline-renderer-build cargo build --locked --release -p csharp-product-runtime "${SCENE_FEATURES[@]}" \
  --bin rusty-scene-render
# rusty compares this commit time with a pair's `sourceTime` to say when it
# is older than the pair it runs.
SOURCE_TIME="$(git log -1 --format=%ct)"
timed cli-build env RUSTY_SOURCE_TIME="$SOURCE_TIME" cargo build --locked --release -p rusty-cli --bin rusty
# A release's product-named launcher (rusty build --release).
timed launcher-build cargo build --locked --release -p product-launcher --bin rusty-launcher
# The Engine services, headless and in process, for products' unit tests
# (Rusty.Engine.Testing.EngineTestHost).
timed test-host-build cargo build --locked --release -p csharp-engine-test-host --lib

stage_started=$SECONDS
install -d "$STAGE/bin" "$STAGE/share/browser/engine/live-debug-panel" "$STAGE/share/browser/engine/video-options" \
  "$STAGE/share/live-debug-client" "$STAGE/share/live-debug-panel" "$STAGE/symbols"
release="${CARGO_TARGET_DIR:-target}/release"
for binary in rusty-product-host rusty-live-debug rusty-scene-render rusty rusty-launcher; do
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
install -m 644 render/artifacts/video-options/index.js \
  "$STAGE/share/browser/engine/video-options/index.js"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/index.html \
  "$STAGE/share/browser/index.html"
install -m 644 render/packages/product-browser-host/runtime-pack-shell/main.js \
  "$STAGE/share/browser/main.js"
find render/packages/live-debug-client/dist -maxdepth 1 -type f \
  ! -name '*.test.*' ! -name '*.tsbuildinfo' \
  -exec install -m 644 {} "$STAGE/share/live-debug-client/" \;
cp -a render/artifacts/live-debug-panel/. "$STAGE/share/live-debug-panel/"
printf 'runtime-pack native/browser staging: %ss\n' "$((SECONDS - stage_started))" >&2

stage_started=$SECONDS
if ((DESKTOP)); then
  # Only what Chromium loads at run time: the library (stripped of its
  # 1.2 GB of debug info), its resources, ICU data, V8 snapshot, and one
  # locale. SwiftShader (Chromium's software GPU) and the
  # bundled Vulkan loader are left out (#8860): the overlay imports Chromium's
  # frames as GPU textures, which needs the same system GPU driver and Vulkan
  # loader the world renderer uses, so neither is ever loaded.
  install -d "$STAGE/lib/cef/locales" "$STAGE/share/third-party/cef"
  CEF_RESOURCES=(chrome_100_percent.pak chrome_200_percent.pak resources.pak icudtl.dat v8_context_snapshot.bin)
  metadata=$(cargo metadata --format-version 1 --locked --all-features)
  # The distribution cef-dll-sys built the host against: CEF_PATH/<CEF
  # version>/<platform> when it downloaded one, else CEF_PATH itself. CEF_PATH
  # can hold other versions beside it, and libcef refuses a host built for a
  # different CEF API, so the packaged one must be that exact version.
  cef_version=$(jq -r '.packages[] | select(.name == "cef-dll-sys") | .version' <<<"$metadata" | head -n 1 | tr -d '\r')
  cef_version=${cef_version#*+}
  if [[ $TARGET == win-x64 ]]; then CEF_PLATFORM=cef_windows_x86_64; else CEF_PLATFORM=cef_linux_x86_64; fi
  CEF_DIST="$CEF_PATH/$cef_version/$CEF_PLATFORM"
  [[ -d "$CEF_DIST" ]] || CEF_DIST="$CEF_PATH"
  dist_version=$(sed -n 's/^#define CEF_VERSION "\([^+"]*\).*/\1/p' "$CEF_DIST/include/cef_version.h" 2>/dev/null | tr -d '\r')
  if [[ $dist_version != "$cef_version" ]]; then
    echo "CEF distribution $CEF_DIST is CEF ${dist_version:-(none found)}; the host was built against CEF $cef_version (cef-dll-sys)" >&2
    exit 1
  fi
  if [[ $TARGET == win-x64 ]]; then
    # Windows CEF carries no debug info in libcef.dll. chrome_elf is loaded
    # with it; d3dcompiler_47 compiles ANGLE's D3D11 shaders, and
    # dxcompiler/dxil compile Dawn's D3D12 shaders.
    for file in libcef.dll chrome_elf.dll d3dcompiler_47.dll dxcompiler.dll dxil.dll; do
      install -m 755 "$CEF_DIST/$file" "$STAGE/lib/cef/$file"
    done
  else
    timed cef-strip strip -o "$STAGE/lib/cef/libcef.so" "$CEF_DIST/libcef.so"
  fi
  for file in "${CEF_RESOURCES[@]}"; do
    install -m 644 "$CEF_DIST/$file" "$STAGE/lib/cef/$file"
  done
  [[ $TARGET == win-x64 ]] || chmod 755 "$STAGE/lib/cef/"*.so
  install -m 644 "$CEF_DIST/locales/en-US.pak" "$STAGE/lib/cef/locales/en-US.pak"
  install -m 644 "$CEF_DIST/CREDITS.html" "$STAGE/share/third-party/cef/CREDITS.html"
  # welding and grafting are MPL-2.0: their source ships with the binary, as
  # fidget-mesh's does.
  for crate in welding grafting; do
    manifest=$(jq -r --arg name "$crate" '.packages[] | select(.name == $name) | .manifest_path' <<<"$metadata" | head -n 1 | tr -d '\r')
    [[ $TARGET != win-x64 ]] || manifest=$(cygpath -u "$manifest")
    [[ -f "$manifest" ]] || { echo "source of $crate not found" >&2; exit 1; }
    cp -a "$(dirname "$manifest")" "$STAGE/share/third-party/$crate"
  done
fi

if ((DESKTOP)); then
  # Servo's MPL-2.0 crates ship unmodified, but for servo-script, whose
  # source ships here (rust/vendor/servo-script/RUSTY_PATCH.md); where each
  # other one's source is.
  install -d "$STAGE/share/third-party/servo"
  cp -a rust/vendor/servo-script "$STAGE/share/third-party/servo-script"
  {
    echo "The window's Servo UI engine includes these MPL-2.0 crates, unmodified."
    echo "Each one's source is at the URL beside it. welding, fidget-mesh and"
    echo "servo-script, the modified MPL-2.0 components, ship their source in"
    echo "share/third-party."
    echo
    cargo tree --locked -p csharp-product-runtime --features servo -e normal \
      --prefix none --format '{p} {l}' 2>/dev/null | tr -d '\r' |
      sed 's/ (\*)$//' |
      awk '$3 !~ /^\(/ { license = $0; sub(/^[^ ]+ [^ ]+ /, "", license) }
           $3 !~ /^\(/ && license ~ /MPL-2\.0/ && license !~ / OR / {print $1, $2}' | sort -u |
      while read -r name version; do
        echo "$name ${version#v} https://crates.io/api/v1/crates/$name/${version#v}/download"
      done
  } > "$STAGE/share/third-party/servo/MPL-2.0-SOURCES.txt"
  if [[ $TARGET == win-x64 ]]; then
    # Servo draws with ANGLE (GLES over D3D11), built with it; the host loads
    # its two DLLs from beside itself.
    for dll in libEGL.dll libGLESv2.dll; do
      built=$(find "$release/build" -path '*mozangle-*' -name "$dll" -printf '%T@ %p\n' | sort -n | tail -n 1 | cut -d' ' -f2-)
      [[ -n $built ]] || { echo "ANGLE's $dll was not built" >&2; exit 1; }
      install -m 755 "$built" "$STAGE/bin/$dll"
    done
    # With Servo the host links nethost dynamically (its static library's C
    # runtime clashes with SpiderMonkey's).
    nethost=$(find "$release/build" -path '*nethost-sys-*' -name nethost.dll -printf '%T@ %p\n' | sort -n | tail -n 1 | cut -d' ' -f2-)
    [[ -n $nethost ]] || { echo "nethost.dll was not downloaded" >&2; exit 1; }
    install -m 755 "$nethost" "$STAGE/bin/nethost.dll"
    angle=$(jq -r '.packages[] | select(.name == "mozangle") | .manifest_path' <<<"$metadata" | head -n 1 | tr -d '\r')
    angle=$(cygpath -u "$(dirname "$angle")")
    install -D -m 644 "$angle/gfx/angle/checkout/LICENSE" "$STAGE/share/third-party/angle/LICENSE"
  fi
fi

if [[ -n $DLSS_SDK_DIR ]]; then
  # NGX loads DLSS from the host's own directory. Its licence asks for the
  # SDK's licence and the notices of the programming guide's section 9.5.
  if [[ $TARGET == win-x64 ]]; then
    dlss_library=("$DLSS_SDK_DIR/lib/Windows_x86_64/rel/nvngx_dlss.dll")
  else
    dlss_library=("$DLSS_SDK_DIR"/lib/Linux_x86_64/rel/libnvidia-ngx-dlss.so.*)
  fi
  [[ ${#dlss_library[@]} == 1 && -f ${dlss_library[0]} ]] || {
    echo "no single DLSS library in $DLSS_SDK_DIR for $TARGET: ${dlss_library[*]}" >&2
    exit 1
  }
  install -m 755 "${dlss_library[0]}" "$STAGE/bin/$(basename "${dlss_library[0]}")"
  install -D -m 644 "$DLSS_SDK_DIR/LICENSE.txt" "$STAGE/share/third-party/nvidia-dlss/LICENSE.txt"
  install -D -m 644 rust/crates/render-wgpu/dlss/NOTICES.txt "$STAGE/share/third-party/nvidia-dlss/NOTICES.txt"
fi

# Include the corresponding source and license for our modified MPL component.
printf 'runtime-pack desktop/vendor staging (includes CEF strip): %ss\n' "$((SECONDS - stage_started))" >&2
install -d "$STAGE/share/third-party"
# The licence and notice texts of the third-party crates the host links and
# the native libraries they build in (Brotli, SpiderMonkey, ANGLE, ...).
timed third-party-notices "${PYTHON3:-python3}" scripts/third-party-notices.py "${HOST_FEATURES[@]}" > "$STAGE/share/third-party/NOTICES.txt"
cp -a rust/vendor/fidget-mesh "$STAGE/share/third-party/fidget-mesh"
# render-wgpu embeds DejaVu Sans; its license travels with the binaries.
install -D -m 0644 rust/crates/render-wgpu/fonts/LICENSE "$STAGE/share/third-party/dejavu-sans/LICENSE"

if [[ $TARGET == win-x64 ]]; then
  # MSVC writes symbols beside each binary as a .pdb.
  for binary in rusty-product-host rusty-live-debug rusty-scene-render rusty rusty-launcher; do
    [[ ! -f "$release/${binary//-/_}.pdb" ]] || install -m 644 "$release/${binary//-/_}.pdb" "$STAGE/symbols/$binary.pdb"
  done
fi

IDENTITY="$("$STAGE/bin/rusty-product-host$EXE" --identity)"
REVISION="$(git rev-parse HEAD)"
# Source/compiler provenance travels with the symbols. Dirty contributor packs
# identify that fact explicitly; published packs should come from a clean commit.
inventory_started=$SECONDS
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
# Hash each staged file once in one process. The inventory is the same one
# consumers already use; spawning sha256sum/awk/wc/tr per source file was a
# substantial part of desktop staging.
"${PYTHON3:-python3}" - "$STAGE" "$TARGET" "$REVISION" "$SOURCE_TIME" "$IDENTITY" > "$STAGE/runtime-manifest.json" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

stage = Path(sys.argv[1])
files = []
for path in sorted(stage.rglob("*"), key=lambda path: path.relative_to(stage).as_posix()):
    if path.name == "runtime-manifest.json" or path.is_symlink() or not path.is_file():
        continue
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    files.append({"path": path.relative_to(stage).as_posix(),
                  "sha256": digest.hexdigest(), "bytes": path.stat().st_size})
json.dump({"artifact": "rusty.product.runtime-pack", "schemaVersion": 1,
           "target": sys.argv[2], "sourceRevision": sys.argv[3],
           "sourceTime": int(sys.argv[4]), "runtime": json.loads(sys.argv[5]),
           "files": files}, sys.stdout, indent=2)
sys.stdout.write("\n")
PY
printf 'runtime-pack inventory: %ss\n' "$((SECONDS - inventory_started))" >&2

mv -- "$STAGE" "$OUTPUT"
trap - EXIT
printf 'runtime pack built: %s\n' "$OUTPUT"
