#!/usr/bin/env bash
set -euo pipefail

# Build the desktop runtime pack for this exact revision and archive it beside
# the C# release pair: rusty-engine-desktop-pack-<version>-<target>.tar.xz and
# its .sha256. It is the pair's runtime pack built with the desktop shell and
# Chromium's runtime (lib/cef), so its ABI fingerprint equals the pair's; the
# default pack stays free of Chromium. `rusty dev` fetches it on the first
# window-output run of a pinned product (#8860).
#
# usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory>
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/.." && pwd)
source "$script_dir/pair-platform.sh"

output=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            [[ $# -ge 2 ]] || { echo "--output needs a directory" >&2; exit 2; }
            output=$2
            shift 2
            ;;
        *)
            echo "usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory>" >&2
            exit 2
            ;;
    esac
done
[[ -n "$output" ]] || { echo "usage: scripts/build-desktop-runtime-pack-archive.sh --output <new-directory>" >&2; exit 2; }
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

"$script_dir/build-runtime-pack.sh" --desktop --output "$stage/$name" >/dev/null
manifest="$stage/$name/runtime-manifest.json"
if [[ "$(jq -r '.sourceRevision' "$manifest")" != "$revision" ]] || [[ ! -f "$stage/$name/lib/cef/$libcef" ]]; then
    echo "RUSTY_ENGINE_DESKTOP_PACK_BUILD: the desktop pack is not this revision's, or has no Chromium runtime" >&2
    exit 1
fi

mkdir "$output"
timestamp=$(git show -s --format=%ct HEAD)
tar --sort=name --mtime="@$timestamp" --owner=0 --group=0 --numeric-owner \
    -C "$stage" -cf - "$name" | xz -T0 -6 > "$output/$name.tar.xz"
(cd "$output" && sha256sum "$name.tar.xz" > "$name.tar.xz.sha256")
printf '%s\n' "$output/$name.tar.xz"
