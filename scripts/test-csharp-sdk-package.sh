#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $(basename "$0") [--coreclr-smoke] [--aot]" >&2
}

# The default remains the broad package-consumer proof.  CI's ordinary C# path
# uses --coreclr-smoke, which keeps the package → generated composition → host
# lifecycle seam without also paying for source-override and NativeAOT fidelity
# coverage.  --aot adds the generated NativeAOT composition to that smoke.
coreclr_smoke=false
aot_requested=false
while (($#)); do
    case "$1" in
        --coreclr-smoke)
            coreclr_smoke=true
            shift
            ;;
        --aot)
            aot_requested=true
            shift
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

run_aot=true
if [[ "$coreclr_smoke" == true && "$aot_requested" != true ]]; then
    run_aot=false
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-sdk-consumer.XXXXXX")
cleanup() {
    if [[ "${RUSTY_ENGINE_SDK_TEST_KEEP_WORK:-}" == "1" ]]; then
        echo "test-csharp-sdk-package: retained disposable consumer at $work_dir" >&2
        return
    fi
    rm -rf -- "$work_dir"
}
trap cleanup EXIT
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
    <EmitCompilerGeneratedFiles>true</EmitCompilerGeneratedFiles>
    <RustyEngineProductEntryType>SdkPackageConsumer.Product</RustyEngineProductEntryType>
    <RustyEngineProductId>fixture.sdk-package</RustyEngineProductId>
    <RustyEngineProductTitle>SDK package fixture</RustyEngineProductTitle>
    <RustyEngineProductUiEntry>main.js</RustyEngineProductUiEntry>
    <RustyEngineProductUiAssets>assets</RustyEngineProductUiAssets>
    <RustyEngineProductLifecycleMode>realtime</RustyEngineProductLifecycleMode>
    <RustyEngineProductFixedStepHz>60</RustyEngineProductFixedStepHz>
    <RustyEngineProductFixedStepMaxCatchUpSteps>4</RustyEngineProductFixedStepMaxCatchUpSteps>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Rusty.Engine" Version="$package_version" />
    <RustyEngineProductInputIntent Include="runtime.exercise" Value="payload:runtime.exercise.payload" />
    <RustyEngineProductInputIntent Include="runtime.exercise.move" Value="digital" />
    <RustyEngineProductInputMapping Include="runtime.exercise.move" Intent="runtime.exercise.move" Trigger="key:key-w:held" />
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

namespace SdkPackageConsumer;

public sealed class Product : IEngineProduct
{
    private readonly IEngineContext _engine;
    private readonly UiStream _stream;
    private readonly Material _voxelMaterial;
    private readonly SpatialSession _spatial;
    private readonly VoxelScenePresentation _voxelPresentation;
    private ulong _sequence;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _stream = _engine.Ui.OpenStream(new UiStreamRequest("sdk-package", "sdk.package.smoke"));
        _voxelMaterial = _engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(0.3f, 0.6f, 0.9f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false));
        _spatial = _engine.Spatial.CreateSession(new SpatialSessionConfig(1, 8, VoxelSurfaceMode.GreedyCubes));
        VoxelSceneReadout scene = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_spatial));
        _engine.Voxel.ApplyEdits(new VoxelEditTransaction(
            _spatial, scene.SourceRevision, new[] { new VoxelEdit(VoxelEditKind.Set, new VoxelAddress(0, 0, 0), 3) }));
        _voxelPresentation = _engine.VoxelScenePresentation.ProjectScene(
            new ProjectVoxelSceneRequest(_spatial, new[] { new VoxelSceneMaterialBinding(3, _voxelMaterial) }));
        PublishUi();
    }

    public void Start() => PublishUi();
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update)
    {
        PublishUi();
        foreach (ProductInputEvent input in update.Input)
        {
            if (input.Kind == InputEventKind.Key
                && input.Keyboard == KeyboardControl.KeyF
                && input.Edge == InputEdge.Pressed)
            {
                return ProductUpdateResult.ReportFault;
            }
        }

        return ProductUpdateResult.None;
    }
    public void Pause() { }
    public void Resume() { }
    public void Restart() => PublishUi();
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => completion.Ticket == 7;
    public void Dispose()
    {
        _voxelPresentation.Dispose();
        _spatial.Dispose();
        _voxelMaterial.Dispose();
        _stream.Dispose();
    }

    private void PublishUi()
    {
        StructuredValueNode[] nodes = [new(StructuredValueKind.Null, 0, 0, 0, 0, 0, 0, 0, 0)];
        _engine.Ui.PublishProjection(new UiProjection(_stream, ++_sequence, new UiValue(nodes, System.Array.Empty<uint>(), 0, System.Array.Empty<byte>())));
    }
}
EOF
mkdir -p "$consumer_dir/product-ui/assets" "$consumer_dir/content"
cat > "$consumer_dir/product-ui/main.js" <<'EOF'
// package-only staged product UI
EOF
printf 'package-only staged content\n' > "$consumer_dir/content/trial.txt"

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
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet msbuild Consumer.csproj -t:StageRustyEngineCoreClrProduct \
            -p:RustyEngineProductBindHost=127.0.0.1 \
            -p:RustyEngineProductPort=40821 \
            -p:RustyEngineProductLiveDebug=true \
            > "$work_dir/coreclr-staging.log" 2>&1
)
if [[ "$coreclr_smoke" != true ]]; then
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
fi

staged_product_directory=$(cd "$consumer_dir" && DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
    dotnet msbuild Consumer.csproj -getProperty:RustyEngineStagedProductDirectory | tail -n 1)
[[ "$staged_product_directory" = /* ]] || {
    echo "test-csharp-sdk-package: staged Product directory was not absolute: $staged_product_directory" >&2
    exit 1
}
[[ -f "$staged_product_directory/product.json" ]] || {
    echo "test-csharp-sdk-package: CoreCLR staging did not emit product.json." >&2
    exit 1
}
[[ -f "$staged_product_directory/coreclr/Rusty.Engine.Product.dll" ]] || {
    echo "test-csharp-sdk-package: CoreCLR staging did not emit the generated composition assembly." >&2
    exit 1
}
if ! find "$consumer_dir/obj/Rusty.Engine/Composition/coreclr/obj" -type f -name 'ProductExports.g.cs' -print -quit | grep -q .; then
    echo "test-csharp-sdk-package: first CoreCLR staging did not generate ProductExports for the composition assembly." >&2
    exit 1
fi
[[ -f "$staged_product_directory/coreclr/Rusty.Engine.Product.runtimeconfig.json" ]] || {
    echo "test-csharp-sdk-package: CoreCLR staging did not emit the generated runtimeconfig." >&2
    exit 1
}
[[ -f "$staged_product_directory/coreclr/Rusty.Engine.dll" && -f "$staged_product_directory/coreclr/Rusty.Engine.Product.deps.json" ]] || {
    echo "test-csharp-sdk-package: CoreCLR staging did not retain its managed dependency closure." >&2
    exit 1
}
[[ -f "$staged_product_directory/ui/main.js" && -f "$staged_product_directory/content/trial.txt" ]] || {
    echo "test-csharp-sdk-package: Product UI/content were not staged." >&2
    exit 1
}
jq -e '.coreclr.assembly == "coreclr/Rusty.Engine.Product.dll" and (.nativeAot | not)' \
    "$staged_product_directory/product.json" >/dev/null || {
    echo "test-csharp-sdk-package: CoreCLR manifest shape is not the V1 Product bundle." >&2
    exit 1
}
jq -e '.server == {"bindHost":"127.0.0.1","port":40821,"liveDebug":true}' \
    "$staged_product_directory/product.json" >/dev/null || {
    echo "test-csharp-sdk-package: SDK staging did not apply the server override properties." >&2
    exit 1
}
jq -e '.lifecycle == {"mode":"realtime","fixedStep":{"hz":60,"maxCatchUpSteps":4}} and .input.intents == [{"id":"runtime.exercise","value":"payload:runtime.exercise.payload"},{"id":"runtime.exercise.move","value":"digital"}] and .input.mappings == [{"id":"runtime.exercise.move","intent":"runtime.exercise.move","trigger":"key:key-w:held"}]' \
    "$staged_product_directory/product.json" >/dev/null || {
    echo "test-csharp-sdk-package: SDK staging did not emit the declared lifecycle/input metadata." >&2
    exit 1
}
if find "$consumer_dir" -path '*/NativeProduct.cs' -o -path '*/NativeProduct.csproj' | grep -q .; then
    echo "test-csharp-sdk-package: ordinary package consumer acquired a checked NativeProduct bridge." >&2
    exit 1
fi
if rg -F -q "$repo_root" "$consumer_dir/obj" "$consumer_packages"; then
    echo "test-csharp-sdk-package: package-only consumer leaked an Engine source path." >&2
    exit 1
fi

if [[ "$coreclr_smoke" == true ]]; then
    host_bundle_dir="$work_dir/host-bundle"
    mkdir -p "$host_bundle_dir"
    printf '<!doctype html><title>Rusty Engine C# package smoke</title>\n' > "$host_bundle_dir/index.html"
    cargo run --manifest-path "$repo_root/Cargo.toml" -p csharp-product-runtime --bin rusty-product-host --locked -- \
        --loader coreclr \
        --library "$staged_product_directory/coreclr/Rusty.Engine.Product.dll" \
        --runtimeconfig "$staged_product_directory/coreclr/Rusty.Engine.Product.runtimeconfig.json" \
        --bundle-dir "$host_bundle_dir" \
        --content-dir "$staged_product_directory/content" \
        --mode realtime \
        --persistence-root "$work_dir/persistence" \
        --content-store-root "$work_dir/content-store" \
        --direct-intent runtime.exercise=payload:runtime.exercise.payload \
        --port 0 \
        --exercise
fi

if [[ "$run_aot" == true ]]; then
    (
        cd "$consumer_dir"
        DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
            dotnet msbuild Consumer.csproj -t:VerifyRustyEngineAot \
                -p:RustyEngineProductBindHost=127.0.0.1 \
                -p:RustyEngineProductPort=40821 \
                -p:RustyEngineProductLiveDebug=true \
                > "$work_dir/nativeaot-staging.log" 2>&1
    )
    if grep -Eiq 'warning (CS|RS)[0-9]+:' "$work_dir/coreclr-staging.log" "$work_dir/nativeaot-staging.log"; then
        echo "test-csharp-sdk-package: generated CoreCLR/NativeAOT composition emitted compiler or analyzer warnings." >&2
        cat "$work_dir/coreclr-staging.log" "$work_dir/nativeaot-staging.log" >&2
        exit 1
    fi
    [[ -f "$staged_product_directory/native/Rusty.Engine.Product.so" ]] || {
        echo "test-csharp-sdk-package: explicit linux-x64 NativeAOT verification did not stage its module." >&2
        exit 1
    }
    jq -e '.nativeAot.module == "native/Rusty.Engine.Product.so" and .coreclr.assembly == "coreclr/Rusty.Engine.Product.dll"' \
        "$staged_product_directory/product.json" >/dev/null || {
        echo "test-csharp-sdk-package: NativeAOT staging did not preserve the same Product bundle." >&2
        exit 1
    }
elif grep -Eiq 'warning (CS|RS)[0-9]+:' "$work_dir/coreclr-staging.log"; then
    echo "test-csharp-sdk-package: generated CoreCLR composition emitted compiler or analyzer warnings." >&2
    cat "$work_dir/coreclr-staging.log" >&2
    exit 1
fi

# The generated composition owns its interop warning baseline. It must not
# become a package-wide NoWarn that hides an ordinary product warning.
if [[ "$coreclr_smoke" == true ]]; then
    echo "csharp SDK CoreCLR package smoke passed"
    exit 0
fi

warning_dir="$work_dir/warning-consumer"
mkdir -p "$warning_dir"
cat > "$warning_dir/WarningConsumer.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><TargetFramework>net10.0</TargetFramework></PropertyGroup>
  <ItemGroup><PackageReference Include="Rusty.Engine" Version="$package_version" /></ItemGroup>
</Project>
EOF
cat > "$warning_dir/Warning.cs" <<'EOF'
namespace SdkPackageWarningConsumer;

public static class WarningSurface
{
    private static int Unassigned;

    public static int Read() => Unassigned;
}
EOF
cat > "$warning_dir/NuGet.Config" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources><clear /><add key="local" value="$feed_dir" /></packageSources>
</configuration>
EOF
(
    cd "$warning_dir"
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet restore WarningConsumer.csproj --configfile NuGet.Config --ignore-failed-sources >/dev/null
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet build WarningConsumer.csproj --no-restore > "$work_dir/product-warning.log" 2>&1
)
grep -Eiq 'warning CS0649:' "$work_dir/product-warning.log" || {
    echo "test-csharp-sdk-package: package-wide warning suppression hid the ordinary product CS0649 warning." >&2
    cat "$work_dir/product-warning.log" >&2
    exit 1
}

echo "csharp SDK package consumer proof passed"
