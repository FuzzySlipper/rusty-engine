#!/usr/bin/env bash
set -euo pipefail

# Publish the exact exercised artifact as release csharp-sdk-v<version>. The
# pair CI workflow runs this after the packaged-consumer check, on the same
# bytes. Discovery is GitHub's Latest release plus the stable pair-release.json
# asset in every release:
#   https://github.com/<repo>/releases/latest/download/pair-release.json
#   https://github.com/<repo>/releases/download/csharp-sdk-v<version>/pair-release.json
# Assets go to a draft first; one edit then publishes the release and moves
# Latest, so a failed upload never changes what is discoverable. The optional
# release-information directory comes from build-csharp-release-info.sh; the
# optional desktop pack from build-desktop-runtime-pack-archive.sh.
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
usage="usage: scripts/publish-csharp-release-pair.sh <pair.tar.gz> [<release-info-directory>] [--desktop <desktop-pack.tar.xz>]"
desktop=""
positional=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help) echo "$usage"; exit 0 ;;
        --desktop)
            [[ $# -ge 2 ]] || { echo "$usage" >&2; exit 2; }
            desktop=$(realpath -- "$2")
            shift 2
            ;;
        *) positional+=("$1"); shift ;;
    esac
done
if [[ ${#positional[@]} -lt 1 || ${#positional[@]} -gt 2 ]]; then
    echo "$usage" >&2
    exit 2
fi
archive=$(realpath -- "${positional[0]}")
info=""
if [[ -n "${positional[1]:-}" ]]; then
    info=$(realpath -- "${positional[1]}")
    [[ -f "$info/release-info.json" ]] || { echo "release information is missing: $info/release-info.json" >&2; exit 1; }
fi
"$script_dir/verify-csharp-release-pair.sh" --archive "$archive"
manifest=$(tar -xOf "$archive" --wildcards '*/pair-manifest.json')
revision=$(jq -r '.sourceRevision' <<<"$manifest")
version=$(jq -r '.package.version' <<<"$manifest")
tag="csharp-sdk-v$version"
archive_name=$(basename -- "$archive")
cd "$script_dir/.."
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)

# A published release keeps its bytes: a rerun or duplicate trigger for the
# same revision leaves it, and discovery, untouched. A leftover draft was
# never discoverable, so it is replaced.
state=$(gh api --paginate "repos/$repo/releases" \
    --jq ".[] | select(.tag_name == \"$tag\") | if .draft then \"draft\" else \"published\" end")
if grep -qx published <<<"$state"; then
    echo "csharp release pair $tag is already published; leaving it and Latest unchanged"
    exit 0
fi
if grep -qx draft <<<"$state"; then
    gh release delete "$tag" --yes
fi

work=$(mktemp -d -t rusty-engine-release.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
assets=("$archive" "$archive.sha256" "$work/pair-release.json")
desktop_pack=null
if [[ -n "$desktop" ]]; then
    desktop_name=$(basename -- "$desktop")
    [[ "$desktop_name" == "rusty-engine-desktop-pack-$version-linux-x64.tar.xz" ]] || {
        echo "the desktop pack $desktop_name is not this pair's ($version)" >&2
        exit 1
    }
    (cd "$(dirname -- "$desktop")" && sha256sum --check --status "$desktop_name.sha256") || {
        echo "the desktop pack $desktop_name does not match its .sha256" >&2
        exit 1
    }
    assets+=("$desktop" "$desktop.sha256")
    desktop_pack=$(jq -n --arg name "$desktop_name" \
        --arg sha256 "$(sha256sum "$desktop" | awk '{print $1}')" \
        --argjson bytes "$(wc -c < "$desktop" | tr -d '[:space:]')" \
        --arg url "https://github.com/$repo/releases/download/$tag/$desktop_name" \
        '{name: $name, sha256: $sha256, bytes: $bytes, url: $url}')
fi
release_info=null
if [[ -n "$info" ]]; then
    download="https://github.com/$repo/releases/download/$tag"
    release_info=$(jq --arg download "$download" '{
        previous,
        notes: ($download + "/" + .releaseNotes),
        apiSurface: ($download + "/" + .apiSurface),
        apiDiff: (if .apiDiff == null then null else $download + "/" + .apiDiff end)
    }' "$info/release-info.json")
    assets+=("$info/release-notes.md" "$info/api-surface.txt")
    [[ ! -f "$info/api-diff.diff" ]] || assets+=("$info/api-diff.diff")
fi
run_url=""
if [[ -n "${GITHUB_RUN_ID:-}" ]]; then
    run_url="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-$repo}/actions/runs/$GITHUB_RUN_ID"
fi
jq -n \
    --arg version "$version" --arg revision "$revision" --arg tag "$tag" \
    --arg name "$archive_name" --arg sha256 "$(sha256sum "$archive" | awk '{print $1}')" \
    --argjson bytes "$(wc -c < "$archive" | tr -d '[:space:]')" \
    --arg url "https://github.com/$repo/releases/download/$tag/$archive_name" \
    --arg run "$run_url" --argjson manifest "$manifest" --argjson info "$release_info" \
    --argjson desktop "$desktop_pack" '{
        artifact: "rusty.engine.csharp-pair-release",
        schemaVersion: 1,
        target: $manifest.target,
        version: $version,
        sourceRevision: $revision,
        tag: $tag,
        archive: {name: $name, sha256: $sha256, bytes: $bytes, url: $url},
        package: {id: $manifest.package.id, version: $manifest.package.version},
        abi: $manifest.runtime.abi,
        desktopPack: $desktop,
        publishedBy: (if $run == "" then null else $run end),
        releaseInfo: $info
    }' > "$work/pair-release.json"
{
    printf 'Verified Linux-x64 Rusty.Engine C# SDK/runtime pair for %s.\n\nThe SDK feed and runtime pack were built together, exercised by a packaged consumer, and published as these same bytes.\n' "$revision"
    [[ -z "$info" ]] || { printf '\n'; cat "$info/release-notes.md"; }
} > "$work/notes.md"

gh release create "$tag" "${assets[@]}" \
    --draft --latest=false --target "$revision" --title "C# SDK/runtime $version" \
    --notes-file "$work/notes.md"

# Latest moves forward only: a late publication of an older revision stays
# available by tag without replacing a newer Latest pair.
latest=true
current=$(gh api "repos/$repo/releases/latest" --jq .tag_name 2>/dev/null || true)
if [[ -n "$current" ]]; then
    relation=$(gh api "repos/$repo/compare/$current...$revision" --jq .status 2>/dev/null || true)
    if [[ "$relation" == behind || "$relation" == identical ]]; then
        latest=false
    fi
fi
gh release edit "$tag" --draft=false --latest="$latest"
printf 'csharp release pair published: %s (latest=%s)\n' "$tag" "$latest"
