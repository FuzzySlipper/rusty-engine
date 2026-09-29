#!/usr/bin/env bash
set -euo pipefail

# Build the release information published with a C# SDK/runtime pair:
#   api-surface.txt   public Rusty.Engine surface of this pair
#   api-diff.diff     that surface against the previous pair (when one exists)
#   release-notes.md  pair identities, authored migration notes, API changes
#   release-info.json the fields publish-csharp-release-pair.sh adds to
#                     pair-release.json
# Authored notes are the "## Migration" sections of docs/evidence/*/README.md
# that changed between the previous pair's revision and this one.

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
pair=""
previous=""
output=""

usage() {
    echo "usage: scripts/build-csharp-release-info.sh --pair <pair.tar.gz> --output <new-directory> [--previous <pair.tar.gz>]" >&2
}

while (($#)); do
    case "$1" in
        --pair|--previous|--output)
            (($# >= 2)) || { usage; exit 2; }
            declare "${1#--}=$2"
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
[[ -n "$pair" && -n "$output" ]] || { usage; exit 2; }
[[ ! -e "$output" ]] || { echo "release info output already exists: $output" >&2; exit 2; }

work=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-release-info.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
dotnet build "$repo_root/csharp/Rusty.Engine.ApiSurface/Rusty.Engine.ApiSurface.csproj" \
    --configuration Release --output "$work/tool" >/dev/null

# Prints the pair manifest and writes the pair's public surface to $2.
read_pair() {
    local archive=$1 surface=$2 dir
    dir=$(mktemp -d "$work/pair.XXXXXX")
    tar -xzf "$archive" -C "$dir" --wildcards '*/pair-manifest.json' '*/sdk-feed/*.nupkg'
    unzip -q -o "$dir"/*/sdk-feed/*.nupkg lib/net10.0/Rusty.Engine.dll -d "$dir/sdk"
    dotnet "$work/tool/Rusty.Engine.ApiSurface.dll" "$dir/sdk/lib/net10.0/Rusty.Engine.dll" > "$surface"
    cat "$dir"/*/pair-manifest.json
}

mkdir -p "$output"
manifest=$(read_pair "$pair" "$output/api-surface.txt")
version=$(jq -r .package.version <<<"$manifest")
revision=$(jq -r .sourceRevision <<<"$manifest")
notes="$output/release-notes.md"

previous_json=null
if [[ -n "$previous" ]]; then
    previous_manifest=$(read_pair "$previous" "$work/previous-surface.txt")
    previous_version=$(jq -r .package.version <<<"$previous_manifest")
    previous_revision=$(jq -r .sourceRevision <<<"$previous_manifest")
    previous_json=$(jq -n --arg v "$previous_version" --arg r "$previous_revision" \
        '{version: $v, sourceRevision: $r, tag: ("csharp-sdk-v" + $v)}')
    # The hunk header names the enclosing public type.
    diff -u --show-function-line='^    public ' \
        --label "Rusty.Engine $previous_version" --label "Rusty.Engine $version" \
        "$work/previous-surface.txt" "$output/api-surface.txt" > "$output/api-diff.diff" || [[ $? == 1 ]]
fi

{
    printf '# Rusty.Engine C# SDK/runtime %s\n\n' "$version"
    printf -- '- Pair: `%s`, source revision `%s`\n' "$version" "$revision"
    if [[ -n "$previous" ]]; then
        printf -- '- Previous published pair: `%s`, source revision `%s`\n' "$previous_version" "$previous_revision"
        printf -- '- These notes cover only the change from the previous pair. When updating across\n  several pairs, read each release'"'"'s notes; `pair-release.json` links each pair\n  to its previous one.\n'
    else
        printf -- '- Previous published pair: none, so there is no API diff or baseline for\n  migration notes.\n'
    fi
    printf '\n## Migration notes\n\n'
    printf 'Authored notes cover behaviour, lifecycle and default changes that a\nsignature diff cannot show.\n\n'
    found=0
    if [[ -n "$previous" ]]; then
        if git -C "$repo_root" cat-file -e "$previous_revision^{commit}" 2>/dev/null \
            || git -C "$repo_root" fetch --quiet --depth=1 origin "$previous_revision" 2>/dev/null; then
            section() { awk '/^## Migration[[:space:]]*$/{p=1;next} /^## /{p=0} p'; }
            while IFS= read -r readme; do
                current=$(git -C "$repo_root" show "$revision:$readme" 2>/dev/null | section || true)
                before=$(git -C "$repo_root" show "$previous_revision:$readme" 2>/dev/null | section || true)
                [[ -n "${current//[[:space:]]/}" && "$current" != "$before" ]] || continue
                found=1
                printf '### %s\n\nFrom [`%s`](https://github.com/FuzzySlipper/rusty-engine/blob/%s/%s).\n%s\n\n' \
                    "$(basename -- "$(dirname -- "$readme")")" "$readme" "$revision" "$readme" "$current"
            done < <(git -C "$repo_root" diff --name-only "$previous_revision" "$revision" -- 'docs/evidence/*/README.md')
            ((found)) || printf 'None were added since the previous pair.\n\n'
        else
            printf 'The previous pair'"'"'s revision is not in this repository'"'"'s history, so\nnotes since it cannot be collected. Check `docs/evidence/*/README.md`.\n\n'
        fi
    else
        printf 'No baseline pair; see the `## Migration` sections in `docs/evidence/*/README.md`.\n\n'
    fi
    printf '## Public API changes\n\n'
    printf 'Generated from the public `Rusty.Engine` assembly surface. It shows signatures\nonly, not behaviour.\n\n'
    if [[ -z "$previous" ]]; then
        printf 'No baseline pair. The full surface is in `api-surface.txt`.\n'
    elif [[ ! -s "$output/api-diff.diff" ]]; then
        printf 'No public signature changes since `%s`.\n' "$previous_version"
    else
        added=$(grep -c '^+[^+]' "$output/api-diff.diff" || true)
        removed=$(grep -c '^-[^-]' "$output/api-diff.diff" || true)
        printf '%s lines added, %s removed; full diff in `api-diff.diff`.\n\n' "$added" "$removed"
        # GitHub caps a release body at 125,000 characters.
        if (($(wc -c < "$output/api-diff.diff") < 60000)); then
            printf '```diff\n'
            cat "$output/api-diff.diff"
            printf '```\n'
        fi
    fi
} > "$notes"

jq -n --argjson previous "$previous_json" --arg diff "$([[ -s "$output/api-diff.diff" ]] && echo api-diff.diff)" '{
    previous: $previous,
    releaseNotes: "release-notes.md",
    apiSurface: "api-surface.txt",
    apiDiff: (if $diff == "" then null else $diff end)
}' > "$output/release-info.json"
[[ -s "$output/api-diff.diff" ]] || rm -f -- "$output/api-diff.diff"
printf 'csharp release info built: %s\n' "$output"
