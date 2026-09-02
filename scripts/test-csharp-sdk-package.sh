#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-sdk-consumer.XXXXXX")
trap 'rm -rf -- "$work_dir"' EXIT
feed_dir="$work_dir/feed"
consumer_dir="$work_dir/consumer"
source_override_dir="$work_dir/source-override"
package_version=0.1.0-sdktest7698

"$repo_root/scripts/pack-csharp-sdk.sh" "$package_version" "$feed_dir" >/dev/null
if "$repo_root/scripts/pack-csharp-sdk.sh" "$package_version" "$feed_dir" >/dev/null 2>&1; then
    echo "test-csharp-sdk-package: pack script overwrote an existing package version." >&2
    exit 1
fi
mkdir -p "$consumer_dir"
package_path="$feed_dir/Rusty.Engine.$package_version.nupkg"

package_entries=$(unzip -Z1 "$package_path")
[[ $(grep -cx 'lib/net10.0/Rusty.Engine.dll' <<<"$package_entries") -eq 1 ]] || {
    echo "test-csharp-sdk-package: package does not contain exactly one Rusty.Engine runtime DLL." >&2
    exit 1
}
[[ $(grep -Ec '^lib/.+\.dll$' <<<"$package_entries") -eq 1 ]] || {
    echo "test-csharp-sdk-package: package split the runtime into additional public assemblies." >&2
    exit 1
}
grep -qx 'buildTransitive/analyzers/Rusty.Engine.ProductGenerator.dll' <<<"$package_entries" || {
    echo "test-csharp-sdk-package: package is missing the product generator analyzer." >&2
    exit 1
}
if grep -q '^analyzers/' <<<"$package_entries"; then
    echo "test-csharp-sdk-package: analyzer is also in NuGet's automatic analyzer path." >&2
    exit 1
fi
grep -qx 'buildTransitive/Rusty.Engine.props' <<<"$package_entries" || {
    echo "test-csharp-sdk-package: package is missing ABI metadata." >&2
    exit 1
}
package_props=$(unzip -p "$package_path" buildTransitive/Rusty.Engine.props)
grep -q '@RUSTY_ENGINE_' <<<"$package_props" && {
    echo "test-csharp-sdk-package: package ABI metadata was not generated from AbiIdentity.g.cs." >&2
    exit 1
}
grep -q 'rusty-engine-sdk/v1' <<<"$package_props" || {
    echo "test-csharp-sdk-package: package ABI metadata does not carry the #7696 identity." >&2
    exit 1
}

cat > "$consumer_dir/NuGet.Config" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources><clear /><add key="local" value="$feed_dir" /><add key="nuget.org" value="https://api.nuget.org/v3/index.json" /></packageSources>
</configuration>
EOF
cat > "$consumer_dir/Consumer.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <OutputType>Library</OutputType>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <EmitCompilerGeneratedFiles>true</EmitCompilerGeneratedFiles>
    <NoWarn>0649</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Rusty.Engine" Version="$package_version" />
  </ItemGroup>
</Project>
EOF
cat > "$consumer_dir/Library.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>false</EnableDefaultCompileItems></PropertyGroup>
  <ItemGroup><Compile Include="Library.cs" /></ItemGroup>
  <ItemGroup><PackageReference Include="Rusty.Engine" Version="$package_version" /></ItemGroup>
</Project>
EOF
cat > "$consumer_dir/Library.cs" <<'EOF'
namespace SdkPackageConsumer;

public static class Library
{
    public static int Value => 1;
}
EOF
mkdir -p "$source_override_dir"
cat > "$source_override_dir/SourceOverride.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
    <RustyEngineUseSourceDevelopment>true</RustyEngineUseSourceDevelopment>
    <RustyEngineSourceDevelopmentPath>$repo_root</RustyEngineSourceDevelopmentPath>
  </PropertyGroup>
  <ItemGroup><Compile Include="Library.cs" /></ItemGroup>
  <ItemGroup><PackageReference Include="Rusty.Engine" Version="$package_version" ExcludeAssets="compile;runtime" /></ItemGroup>
</Project>
EOF
cp "$consumer_dir/Library.cs" "$source_override_dir/Library.cs"
cat > "$consumer_dir/Product.cs" <<'EOF'
using Rusty.Engine;

[assembly: EngineProduct(typeof(SdkPackageConsumer.Product))]

namespace SdkPackageConsumer;

public sealed class Product : IEngineProduct
{
    public Product(ProductCreateContext context) { }
    public void Start() { }
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
}
EOF

# The only available package source is the fresh local feed. The SDK's source
# tree is not an input to restore or build; consumer assets must not name it.
consumer_home="$work_dir/dotnet-home"
consumer_packages="$work_dir/packages"
mkdir -p "$consumer_home" "$consumer_packages"
(
    cd "$consumer_dir"
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet restore Consumer.csproj --configfile NuGet.Config --ignore-failed-sources
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet restore Library.csproj --configfile NuGet.Config --ignore-failed-sources
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet build Library.csproj --no-restore
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet build Consumer.csproj --no-restore
)
(
    cd "$source_override_dir"
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet restore SourceOverride.csproj --configfile "$consumer_dir/NuGet.Config" --ignore-failed-sources
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet build SourceOverride.csproj --no-restore
)
jq -e --arg package "Rusty.Engine/$package_version" \
    '.targets["net10.0"][$package] | ((.compile // {} | keys | all(.[]; endswith("/_._"))) and (.runtime // {} | keys | all(.[]; endswith("/_._"))))' \
    "$source_override_dir/obj/project.assets.json" >/dev/null || {
    echo "test-csharp-sdk-package: source override retained package compile/runtime assets." >&2
    exit 1
}

rg -q 'BindV1' "$consumer_dir/obj" || {
    echo "test-csharp-sdk-package: ProductGenerator did not emit BindV1 from the package." >&2
    exit 1
}
rg -q 'rusty-engine-sdk/v1' "$consumer_dir/obj" || {
    echo "test-csharp-sdk-package: generated product identity does not match #7696." >&2
    exit 1
}
if rg -F -q "$repo_root" "$consumer_dir/obj" "$consumer_packages"; then
    echo "test-csharp-sdk-package: package-only consumer leaked an Engine source path." >&2
    exit 1
fi

echo "csharp SDK package consumer proof passed"
