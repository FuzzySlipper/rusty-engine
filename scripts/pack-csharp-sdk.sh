#!/usr/bin/env bash
set -euo pipefail

# Build one immutable local Rusty.Engine package. Generation happens here,
# before either SDK project is built; consumers receive only the resulting DLL
# and analyzer, never this script or its Rust/Clang toolchain.

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "usage: pack-csharp-sdk.sh <package-version> [local-feed-directory]" >&2
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
package_version=$1
feed_dir=${2:-"$repo_root/artifacts/csharp-sdk-feed"}
repository_commit=$(git -C "$repo_root" rev-parse HEAD)
repository_url=$(git -C "$repo_root" remote get-url origin 2>/dev/null || true)
if [[ -z "$repository_url" ]]; then
    echo "pack-csharp-sdk: origin remote is required to embed immutable package repository metadata" >&2
    exit 1
fi

if [[ "$feed_dir" != /* ]]; then
    feed_dir="$repo_root/$feed_dir"
fi

package_path="$feed_dir/Rusty.Engine.$package_version.nupkg"
if [[ -e "$package_path" ]]; then
    echo "pack-csharp-sdk: refusing to overwrite immutable package $package_path" >&2
    exit 1
fi

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-sdk-pack.XXXXXX")
trap 'rm -rf -- "$work_dir"' EXIT
generated_dir="$work_dir/generated"
generated_inputs_dir="$generated_dir/GeneratedInputs"
package_metadata_dir="$work_dir/package-metadata"

dotnet build "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" --no-restore
"$repo_root/scripts/generate-csharp-native-bindings.sh" "$generated_dir" "$generated_inputs_dir"

identity_input="$generated_inputs_dir/AbiIdentity.g.cs"
sdk_identity=$(sed -n 's/.*SdkBuildIdentity = "\([^"]*\)";.*/\1/p' "$identity_input")
protocol_version=$(sed -n 's/.*ProtocolVersion = \([0-9][0-9]*\);.*/\1/p' "$identity_input")
fingerprint=$(sed -n 's/.*word[0-9] = 0x\([0-9A-F][0-9A-F]*\)UL,.*/\1/p' "$identity_input" | tr -d '\n')
if [[ -z "$sdk_identity" || -z "$protocol_version" || ${#fingerprint} -ne 64 ]]; then
    echo "pack-csharp-sdk: could not read the generated product ABI identity" >&2
    exit 1
fi
mkdir -p "$package_metadata_dir"
sed \
    -e "s|@RUSTY_ENGINE_SDK_BUILD_IDENTITY@|$sdk_identity|g" \
    -e "s|@RUSTY_ENGINE_PRODUCT_ABI_PROTOCOL_VERSION@|$protocol_version|g" \
    -e "s|@RUSTY_ENGINE_PRODUCT_ABI_FINGERPRINT@|$fingerprint|g" \
    -e "s|@RUSTY_ENGINE_SDK_PACKAGE_VERSION@|$package_version|g" \
    "$repo_root/csharp/Rusty.Engine/buildTransitive/Rusty.Engine.props.in" \
    > "$package_metadata_dir/Rusty.Engine.props"

mkdir -p "$feed_dir"
dotnet pack "$repo_root/csharp/Rusty.Engine/Rusty.Engine.csproj" --no-restore --configuration Release \
    -p:PackageVersion="$package_version" \
    -p:PackageOutputPath="$feed_dir" \
    -p:RustyEngineGeneratedBindingsDir="$generated_dir" \
    -p:RustyEngineGeneratedInputsDir="$generated_inputs_dir" \
    -p:RustyEnginePackageMetadataDir="$package_metadata_dir" \
    -p:RustyEnginePackageAbiIdentity="$sdk_identity" \
    -p:RepositoryType=git \
    -p:RepositoryUrl="$repository_url" \
    -p:RepositoryCommit="$repository_commit" \
    -p:PublishRepositoryUrl=true \
    -p:RustyEngineGenerateBindings=false

if [[ ! -f "$package_path" ]]; then
    echo "pack-csharp-sdk: expected package was not produced: $package_path" >&2
    exit 1
fi

echo "$package_path"
