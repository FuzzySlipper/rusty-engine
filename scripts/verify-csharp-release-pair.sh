#!/usr/bin/env bash
set -euo pipefail

# Verify a released C# SDK/runtime pair without consulting an Engine checkout.
# The package, runtime manifest, host identity, and every payload hash must
# describe the same immutable source revision and ABI before it is consumed.

usage() {
    echo "usage: $(basename "$0") (--archive <pair.tar.gz> | --directory <extracted-pair>)" >&2
}

archive=""
pair_root=""
temporary=""
cleanup() {
    [[ -z "$temporary" ]] || rm -rf -- "$temporary"
}
trap cleanup EXIT

while (($#)); do
    case "$1" in
        --archive)
            (($# >= 2)) || { usage; exit 2; }
            archive=$2
            shift 2
            ;;
        --directory)
            (($# >= 2)) || { usage; exit 2; }
            pair_root=$2
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

if [[ -n "$archive" && -n "$pair_root" ]] || [[ -z "$archive" && -z "$pair_root" ]]; then
    usage
    exit 2
fi

fail() {
    echo "RUSTY_ENGINE_PAIR_$1: $2" >&2
    exit 1
}

if [[ -n "$archive" ]]; then
    [[ -f "$archive" ]] || fail ARCHIVE "pair archive is missing: $archive"
    checksum="$archive.sha256"
    [[ -f "$checksum" ]] || fail CHECKSUM "pair checksum is missing: $checksum"
    expected=$(awk 'NF { print $1; exit }' "$checksum")
    actual=$(sha256sum "$archive" | awk '{print $1}')
    [[ "$expected" == "$actual" ]] || fail CHECKSUM "pair archive checksum does not match $checksum"
    temporary=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-pair-verify.XXXXXX")
    tar -xzf "$archive" -C "$temporary" || fail ARCHIVE "pair archive could not be extracted"
    mapfile -t roots < <(find "$temporary" -mindepth 1 -maxdepth 1 -type d -print)
    [[ ${#roots[@]} -eq 1 ]] || fail ARCHIVE "pair archive must contain exactly one root directory"
    pair_root=${roots[0]}
fi

[[ -d "$pair_root" ]] || fail LAYOUT "pair directory is missing: $pair_root"
manifest="$pair_root/pair-manifest.json"
[[ -f "$manifest" ]] || fail LAYOUT "pair-manifest.json is missing"
jq -e '.artifact == "rusty.engine.csharp-pair" and .schemaVersion == 1 and .target == "linux-x64"' "$manifest" >/dev/null \
    || fail MANIFEST "pair manifest is not a supported Linux-x64 C# pair"

while IFS=$'\t' read -r path digest bytes; do
    [[ -n "$path" && "$path" != /* && "$path" != *".."* ]] || fail PAYLOAD "pair manifest names an unsafe payload path"
    file="$pair_root/$path"
    [[ -f "$file" ]] || fail PAYLOAD "pair payload is missing: $path"
    [[ $(sha256sum "$file" | awk '{print $1}') == "$digest" ]] || fail PAYLOAD "pair payload hash does not match: $path"
    [[ $(wc -c < "$file" | tr -d '[:space:]') == "$bytes" ]] || fail PAYLOAD "pair payload size does not match: $path"
done < <(jq -r '.payload[] | [.path, .sha256, (.bytes | tostring)] | @tsv' "$manifest")

payload_count=$(jq '.payload | length' "$manifest")
actual_count=$(find "$pair_root" -type f ! -name pair-manifest.json | wc -l | tr -d '[:space:]')
[[ "$payload_count" == "$actual_count" ]] || fail PAYLOAD "pair manifest does not account for every payload file"

package_path=$(jq -r '.package.path' "$manifest")
runtime_path=$(jq -r '.runtime.path' "$manifest")
sdk_package="$pair_root/$package_path"
runtime_pack="$pair_root/$runtime_path"
[[ -f "$sdk_package" && -d "$runtime_pack" ]] || fail LAYOUT "pair SDK package or runtime pack is missing"

nuspec_entry=$(unzip -Z1 "$sdk_package" | sed -n 's|^\(.*\.nuspec\)$|\1|p' | head -n 1)
[[ -n "$nuspec_entry" ]] || fail SDK_METADATA "SDK package has no nuspec"
nuspec=$(unzip -p "$sdk_package" "$nuspec_entry")
props=$(unzip -p "$sdk_package" buildTransitive/Rusty.Engine.props) || fail SDK_METADATA "SDK package is missing generated metadata"
package_id=$(sed -n 's|.*<id>\([^<]*\)</id>.*|\1|p' <<<"$nuspec")
package_version=$(sed -n 's|.*<version>\([^<]*\)</version>.*|\1|p' <<<"$nuspec")
package_commit=$(sed -n 's/.*<repository[^>]* commit="\([^"]*\)".*/\1/p' <<<"$nuspec")
package_repository_type=$(sed -n 's/.*<repository[^>]* type="\([^"]*\)".*/\1/p' <<<"$nuspec")
package_repository_url=$(sed -n 's/.*<repository[^>]* url="\([^"]*\)".*/\1/p' <<<"$nuspec")
sdk_identity=$(sed -n 's|.*<RustyEngineSdkBuildIdentity>\([^<]*\)</RustyEngineSdkBuildIdentity>.*|\1|p' <<<"$props")
sdk_protocol=$(sed -n 's|.*<RustyEngineProductAbiProtocolVersion>\([^<]*\)</RustyEngineProductAbiProtocolVersion>.*|\1|p' <<<"$props")
sdk_fingerprint=$(sed -n 's|.*<RustyEngineProductAbiFingerprint>\([^<]*\)</RustyEngineProductAbiFingerprint>.*|\1|p' <<<"$props")
sdk_fingerprint=${sdk_fingerprint,,}
sdk_version=$(sed -n 's|.*<RustyEngineSdkPackageVersion>\([^<]*\)</RustyEngineSdkPackageVersion>.*|\1|p' <<<"$props")

[[ "$package_id" == "Rusty.Engine" ]] || fail SDK_METADATA "SDK package ID is not Rusty.Engine"
[[ "$package_version" == "$(jq -r '.package.version' "$manifest")" && "$sdk_version" == "$package_version" ]] \
    || fail SDK_METADATA "SDK package version does not match pair manifest"
[[ "$package_commit" == "$(jq -r '.sourceRevision' "$manifest")" ]] \
    || fail IDENTITY "SDK repository commit does not match pair source revision"
[[ "$package_repository_type" == "$(jq -r '.package.repositoryType' "$manifest")" && "$package_repository_url" == "$(jq -r '.package.repositoryUrl' "$manifest")" ]] \
    || fail SDK_METADATA "SDK repository metadata does not match pair manifest"
[[ "$sdk_identity" == "$(jq -r '.package.sdkBuildIdentity' "$manifest")" && "$sdk_protocol" == "$(jq -r '.package.protocolVersion' "$manifest")" && "$sdk_fingerprint" == "$(jq -r '.package.fingerprint' "$manifest")" ]] \
    || fail IDENTITY "SDK generated ABI metadata does not match pair manifest"

runtime_manifest="$runtime_pack/runtime-manifest.json"
[[ -f "$runtime_manifest" ]] || fail RUNTIME_METADATA "runtime-manifest.json is missing"
jq -e '.artifact == "rusty.product.runtime-pack" and .schemaVersion == 1 and .target == "linux-x64"' "$runtime_manifest" >/dev/null \
    || fail RUNTIME_METADATA "runtime manifest is not a supported Linux-x64 runtime pack"
[[ $(jq -r '.sourceRevision' "$runtime_manifest") == "$(jq -r '.sourceRevision' "$manifest")" ]] \
    || fail IDENTITY "runtime source revision does not match pair source revision"
jq -e --slurpfile pair "$manifest" '
  .sourceRevision == $pair[0].runtime.sourceRevision and
  .runtime.abi == $pair[0].runtime.abi and
  (.runtime.abi.protocolVersion | tostring) == ($pair[0].package.protocolVersion | tostring) and
  .runtime.abi.fingerprint == $pair[0].package.fingerprint
' "$runtime_manifest" >/dev/null || fail IDENTITY "runtime ABI does not match SDK ABI metadata"

host="$runtime_pack/bin/rusty-product-host"
[[ -x "$host" ]] || fail RUNTIME_HOST "runtime host is missing or not executable"
host_identity=$($host --identity) || fail RUNTIME_HOST "runtime host --identity failed"
jq -e --argjson host "$host_identity" '.runtime == $host' "$runtime_manifest" >/dev/null \
    || fail RUNTIME_HOST "runtime host identity does not match runtime manifest"

while IFS=$'\t' read -r path digest bytes; do
    file="$runtime_pack/$path"
    [[ -f "$file" ]] || fail RUNTIME_PAYLOAD "runtime manifest payload is missing: $path"
    [[ $(sha256sum "$file" | awk '{print $1}') == "$digest" ]] || fail RUNTIME_PAYLOAD "runtime manifest payload hash does not match: $path"
    [[ $(wc -c < "$file" | tr -d '[:space:]') == "$bytes" ]] || fail RUNTIME_PAYLOAD "runtime manifest payload size does not match: $path"
done < <(jq -r '.files[] | [.path, .sha256, (.bytes | tostring)] | @tsv' "$runtime_manifest")

printf 'csharp release pair verified: %s\n' "$pair_root"
