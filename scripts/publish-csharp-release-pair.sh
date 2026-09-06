#!/usr/bin/env bash
set -euo pipefail

# Publish the exact locally verified artifact. GitHub Actions verifies source;
# it does not rebuild or publish a competing pair when this creates the tag.
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ $# != 1 || "$1" == --help ]]; then
    echo "usage: scripts/publish-csharp-release-pair.sh <pair.tar.gz>"
    [[ ${1:-} == --help ]] && exit 0
    exit 2
fi
archive=$(realpath -- "$1")
"$script_dir/verify-csharp-release-pair.sh" --archive "$archive"
manifest=$(tar -xOf "$archive" --wildcards '*/pair-manifest.json')
revision=$(jq -r '.sourceRevision' <<<"$manifest")
version=$(jq -r '.package.version' <<<"$manifest")
notes=$(mktemp -t rusty-engine-release-notes.XXXXXX)
trap 'rm -f -- "$notes"' EXIT
printf 'Verified Linux-x64 Rusty.Engine C# SDK/runtime pair for %s.\n\nSDK and runtime are published together from the same local artifact.\n' "$revision" > "$notes"
cd "$script_dir/.."
# gh release create refuses an existing release. Never use upload --clobber:
# a published pair keeps its bytes, even when another machine could rebuild it.
gh release create "csharp-sdk-v$version" "$archive" "$archive.sha256" \
    --target "$revision" --title "C# SDK/runtime $version" \
    --notes-file "$notes" --prerelease
