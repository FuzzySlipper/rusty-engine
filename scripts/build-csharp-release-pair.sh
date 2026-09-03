#!/usr/bin/env bash
set -euo pipefail

# Build one immutable Linux-x64 C# SDK/runtime distribution pair.  The pair is
# intentionally derived from the checked-out commit rather than a caller's
# version string, so it cannot silently relabel uncommitted or mismatched bits.

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
output=""

usage() {
    echo "usage: scripts/build-csharp-release-pair.sh --output <new-directory>" >&2
}

while (($#)); do
    case "$1" in
        --output)
            (($# >= 2)) || { usage; exit 2; }
            output=$2
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

[[ -n "$output" ]] || { usage; exit 2; }
if [[ "$output" != /* ]]; then
    output="$repo_root/$output"
fi

cd "$repo_root"
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "RUSTY_ENGINE_PAIR_DIRTY_CHECKOUT: exact pair publication requires a clean checkout; commit, stash, or remove every tracked and untracked change first" >&2
    exit 1
fi
if [[ -e "$output" ]]; then
    echo "RUSTY_ENGINE_PAIR_OUTPUT_EXISTS: refusing to overwrite immutable pair output $output" >&2
    exit 1
fi

revision=$(git rev-parse HEAD)
short_revision=$(git rev-parse --short=12 HEAD)
version="0.1.0-dev.$short_revision"
pair_name="rusty-engine-csharp-pair-$version-linux-x64"
archive_name="$pair_name.tar.gz"
archive_path="$output/$archive_name"
checksum_path="$archive_path.sha256"

mkdir -p "$(dirname "$output")"
stage=$(mktemp -d "$(dirname "$output")/.csharp-release-pair.XXXXXX")
cleanup() { rm -rf -- "$stage"; }
trap cleanup EXIT

pair_root="$stage/$pair_name"
sdk_feed="$pair_root/sdk-feed"
runtime_pack="$pair_root/runtime-pack"
mkdir -p "$sdk_feed"

sdk_package=$("$script_dir/pack-csharp-sdk.sh" "$version" "$sdk_feed" | tail -n 1)
[[ -f "$sdk_package" ]] || {
    echo "RUSTY_ENGINE_PAIR_SDK_BUILD: SDK pack builder did not produce its declared package" >&2
    exit 1
}
"$script_dir/build-runtime-pack.sh" --output "$runtime_pack" >/dev/null
install -m 755 "$script_dir/verify-csharp-release-pair.sh" "$pair_root/verify-pair.sh"

runtime_manifest="$runtime_pack/runtime-manifest.json"
[[ -f "$runtime_manifest" ]] || {
    echo "RUSTY_ENGINE_PAIR_RUNTIME_BUILD: runtime pack did not produce runtime-manifest.json" >&2
    exit 1
}

nuspec_entry=$(unzip -Z1 "$sdk_package" | sed -n 's|^\(.*\.nuspec\)$|\1|p' | head -n 1)
[[ -n "$nuspec_entry" ]] || {
    echo "RUSTY_ENGINE_PAIR_SDK_METADATA: SDK package has no nuspec" >&2
    exit 1
}
nuspec=$(unzip -p "$sdk_package" "$nuspec_entry")
package_commit=$(sed -n 's/.*<repository[^>]* commit="\([^"]*\)".*/\1/p' <<<"$nuspec")
package_repository_type=$(sed -n 's/.*<repository[^>]* type="\([^"]*\)".*/\1/p' <<<"$nuspec")
package_repository_url=$(sed -n 's/.*<repository[^>]* url="\([^"]*\)".*/\1/p' <<<"$nuspec")
package_version=$(sed -n 's|.*<version>\([^<]*\)</version>.*|\1|p' <<<"$nuspec")
package_id=$(sed -n 's|.*<id>\([^<]*\)</id>.*|\1|p' <<<"$nuspec")
package_props=$(unzip -p "$sdk_package" buildTransitive/Rusty.Engine.props)
sdk_identity=$(sed -n 's|.*<RustyEngineSdkBuildIdentity>\([^<]*\)</RustyEngineSdkBuildIdentity>.*|\1|p' <<<"$package_props")
sdk_protocol=$(sed -n 's|.*<RustyEngineProductAbiProtocolVersion>\([^<]*\)</RustyEngineProductAbiProtocolVersion>.*|\1|p' <<<"$package_props")
sdk_fingerprint=$(sed -n 's|.*<RustyEngineProductAbiFingerprint>\([^<]*\)</RustyEngineProductAbiFingerprint>.*|\1|p' <<<"$package_props")
sdk_fingerprint=${sdk_fingerprint,,}
sdk_version=$(sed -n 's|.*<RustyEngineSdkPackageVersion>\([^<]*\)</RustyEngineSdkPackageVersion>.*|\1|p' <<<"$package_props")
runtime_revision=$(jq -r '.sourceRevision // empty' "$runtime_manifest")
runtime_protocol=$(jq -r '.runtime.abi.protocolVersion // empty' "$runtime_manifest")
runtime_fingerprint=$(jq -r '.runtime.abi.fingerprint // empty' "$runtime_manifest")

if [[ "$package_id" != Rusty.Engine || "$package_version" != "$version" || "$sdk_version" != "$version" || "$package_commit" != "$revision" || "$package_repository_type" != git || -z "$package_repository_url" || "$runtime_revision" != "$revision" || "$sdk_protocol" != "$runtime_protocol" || "$sdk_fingerprint" != "$runtime_fingerprint" ]]; then
    echo "RUSTY_ENGINE_PAIR_IDENTITY: SDK and runtime identities do not match the exact source revision" >&2
    exit 1
fi

pair_manifest="$pair_root/pair-manifest.json"
{
    printf '{\n'
    printf '  "artifact": "rusty.engine.csharp-pair",\n'
    printf '  "schemaVersion": 1,\n'
    printf '  "target": "linux-x64",\n'
    printf '  "sourceRevision": "%s",\n' "$revision"
    printf '  "package": {"id":"Rusty.Engine","version":"%s","path":"sdk-feed/Rusty.Engine.%s.nupkg","repositoryType":"%s","repositoryUrl":"%s","repositoryCommit":"%s","sdkBuildIdentity":"%s","protocolVersion":%s,"fingerprint":"%s"},\n' "$version" "$version" "$package_repository_type" "$package_repository_url" "$package_commit" "$sdk_identity" "$sdk_protocol" "$sdk_fingerprint"
    printf '  "runtime": {"path":"runtime-pack","sourceRevision":"%s","abi":%s},\n' "$runtime_revision" "$(jq -c '.runtime.abi' "$runtime_manifest")"
    printf '  "payload": [\n'
    first=1
    while IFS= read -r file; do
        relative=${file#"$pair_root/"}
        digest=$(sha256sum "$file" | awk '{print $1}')
        bytes=$(wc -c < "$file" | tr -d '[:space:]')
        if ((first)); then first=0; else printf ',\n'; fi
        printf '    {"path":"%s","sha256":"%s","bytes":%s}' "$relative" "$digest" "$bytes"
    done < <(find "$sdk_feed" "$runtime_pack" "$pair_root/verify-pair.sh" -type f -print | LC_ALL=C sort)
    printf '\n  ]\n}\n'
} > "$pair_manifest"

"$script_dir/verify-csharp-release-pair.sh" --directory "$pair_root" >/dev/null

mkdir "$output"
timestamp=$(git show -s --format=%ct HEAD)
tar --sort=name --mtime="@$timestamp" --owner=0 --group=0 --numeric-owner \
    -C "$stage" -czf "$archive_path" "$pair_name"
printf '%s  %s\n' "$(sha256sum "$archive_path" | awk '{print $1}')" "$archive_name" > "$checksum_path"

printf 'csharp release pair built: %s\n' "$archive_path"
printf 'checksum: %s\n' "$checksum_path"
