#!/usr/bin/env bash
set -euo pipefail

# Build the desktop runtime pack for this exact revision and archive it beside
# the C# release pair: rusty-engine-desktop-pack-<version>-<target>.tar.xz and
# its .sha256. It is the pair's runtime pack built with the desktop shell, both
# UI engines (Chromium, with its runtime in lib/cef, and Servo) so its ABI
# fingerprint equals the pair's; the default pack stays free of either.
# Servo's build needs Python 3. `rusty dev` fetches it on the first
# window-output run of a pinned product (#8860).
#
# usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory> [--dlss <dlss-sdk>]
#
# --dlss builds it with NVIDIA DLSS from that SDK checkout
# (scripts/build-runtime-pack.sh --dlss).
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/.." && pwd)
source "$script_dir/pair-platform.sh"

output=""
dlss=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            [[ $# -ge 2 ]] || { echo "--output needs a directory" >&2; exit 2; }
            output=$2
            shift 2
            ;;
        --dlss)
            [[ $# -ge 2 ]] || { echo "--dlss needs the DLSS SDK directory" >&2; exit 2; }
            dlss=(--dlss "$2")
            shift 2
            ;;
        *)
            echo "usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory> [--dlss <dlss-sdk>]" >&2
            exit 2
            ;;
    esac
done
[[ -n "$output" ]] || { echo "usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory> [--dlss <dlss-sdk>]" >&2; exit 2; }
[[ "$output" == /* ]] || output="$repo_root/$output"

cd "$repo_root"
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "RUSTY_ENGINE_DESKTOP_PACK_DIRTY_CHECKOUT: the desktop pack is published beside a pair and needs a clean checkout" >&2
    exit 1
fi
if [[ -e "$output" ]]; then
    echo "RUSTY_ENGINE_DESKTOP_PACK_OUTPUT_EXISTS: refusing to overwrite $output" >&2
    exit 1
fi

revision=$(git rev-parse HEAD)
version="0.1.0-dev.$(git rev-parse --short=12 HEAD)"
name="rusty-engine-desktop-pack-$version-$pair_target"
libcef=libcef.so
[[ $pair_target != win-x64 ]] || libcef=libcef.dll
mkdir -p "$(dirname "$output")"
stage=$(mktemp -d "$(dirname "$output")/.desktop-pack.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT

"$script_dir/build-runtime-pack.sh" --desktop "${dlss[@]}" --output "$stage/$name" >/dev/null
manifest="$stage/$name/runtime-manifest.json"
if [[ "$(jq -r '.sourceRevision' "$manifest")" != "$revision" ]] || [[ ! -f "$stage/$name/lib/cef/$libcef" ]]; then
    echo "RUSTY_ENGINE_DESKTOP_PACK_BUILD: the desktop pack is not this revision's, or has no Chromium runtime" >&2
    exit 1
fi

mkdir "$output"
install -m 644 "$manifest" "$output/runtime-manifest.json"
timestamp=$(git show -s --format=%ct HEAD)
archive_started=$SECONDS
tar --sort=name --mtime="@$timestamp" --owner=0 --group=0 --numeric-owner \
    -C "$stage" -cf - "$name" | xz -T0 -1 > "$output/$name.tar.xz"
printf 'desktop archive compression: %ss\n' "$((SECONDS - archive_started))" >&2
digest_started=$SECONDS
(cd "$output" && sha256sum "$name.tar.xz" > "$name.tar.xz.sha256")
printf 'desktop archive digest: %ss\n' "$((SECONDS - digest_started))" >&2
printf '%s\n' "$output/$name.tar.xz"
