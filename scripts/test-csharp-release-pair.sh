#!/usr/bin/env bash
set -euo pipefail

# Focused external-consumer proof for a published C# SDK/runtime pair.  The
# fixture lives entirely outside the checkout and restores Rusty.Engine only
# from the pair's embedded feed, so Cargo, generator inputs, and Engine browser
# files cannot be accidental product dependencies. Standard .NET platform
# reference packs may still resolve from NuGet on machines that do not bundle
# them with the installed SDK.

if [[ $# -lt 1 || $# -gt 2 || (${2:-} != "" && ${2:-} != --aot) ]]; then
    echo "usage: scripts/test-csharp-release-pair.sh <pair.tar.gz> [--aot]" >&2
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
archive=$1
run_aot=${2:-}
"$script_dir/verify-csharp-release-pair.sh" --archive "$archive" >/dev/null

work=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-pair-consumer.XXXXXX")
host_pid=""
control_open=0
cleanup() {
    if [[ "$control_open" == 1 ]]; then exec 9>&-; control_open=0; fi
    if [[ -n "$host_pid" ]] && kill -0 "$host_pid" 2>/dev/null; then
        kill "$host_pid" 2>/dev/null || true
        wait "$host_pid" 2>/dev/null || true
    fi
    if [[ "${RUSTY_ENGINE_PAIR_TEST_KEEP_WORK:-}" == "1" ]]; then
        echo "test-csharp-release-pair: retained disposable consumer at $work" >&2
    else
        rm -rf -- "$work"
    fi
}
trap cleanup EXIT

tar -xzf "$archive" -C "$work"
pair_root=$(find "$work" -mindepth 1 -maxdepth 1 -type d -print -quit)
[[ -n "$pair_root" ]] || { echo "RUSTY_ENGINE_PAIR_TEST_LAYOUT: pair archive was empty" >&2; exit 1; }
"$pair_root/verify-pair.sh" --directory "$pair_root" >/dev/null
version=$(jq -r '.package.version' "$pair_root/pair-manifest.json")
feed="$pair_root/sdk-feed"
runtime="$pair_root/runtime-pack"
consumer="$work/consumer"
mkdir -p "$consumer/product-ui/assets" "$consumer/content"

cat > "$consumer/NuGet.Config" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<configuration><packageSources><clear /><add key="rusty-engine-pair" value="$feed" /><add key="nuget.org" value="https://api.nuget.org/v3/index.json" /></packageSources></configuration>
EOF
cat > "$consumer/PairConsumer.csproj" <<EOF
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <OutputType>Library</OutputType>
    <ImplicitUsings>enable</ImplicitUsings>
    <RustyEngineProductEntryType>PairConsumer.Product</RustyEngineProductEntryType>
    <RustyEngineProductId>fixture.release-pair</RustyEngineProductId>
    <RustyEngineProductTitle>Release pair fixture</RustyEngineProductTitle>
    <RustyEngineProductLifecycleMode>realtime</RustyEngineProductLifecycleMode>
    <RustyEngineProductFixedStepHz>60</RustyEngineProductFixedStepHz>
    <RustyEngineProductFixedStepMaxCatchUpSteps>4</RustyEngineProductFixedStepMaxCatchUpSteps>
    <RustyEngineProductInputCursorMode>unlocked</RustyEngineProductInputCursorMode>
    <RustyEngineProductLiveDebug>true</RustyEngineProductLiveDebug>
    <RustyEngineProductUiProjectionStream>pair.ui</RustyEngineProductUiProjectionStream>
    <RustyEngineProductUiProjectionContract>pair.ui.v1</RustyEngineProductUiProjectionContract>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Rusty.Engine" Version="$version" />
    <RustyEngineProductInputIntent Include="pair.use" Value="digital" />
    <RustyEngineProductInputMapping Include="pair.use.key" Intent="pair.use" Trigger="key:key-e:pressed" />
  </ItemGroup>
</Project>
EOF
cat > "$consumer/Product.cs" <<'EOF'
using Rusty.Engine;

namespace PairConsumer;

public sealed class Product : IEngineProduct
{
    private readonly RenderOutputChecks _outputs;
    private readonly UiStream _ui;
    public Product(ProductCreateContext context)
    {
        _ui = context.Engine.Ui.OpenStream(new("pair.ui", "pair.ui.v1"));
        context.Engine.Ui.PublishProjection(new(_ui, 1, new UiValue(
            new StructuredValueNode[] {
                new(StructuredValueKind.Object, 0, 0, 0, 0, 0, 0, 0, 1),
                new(StructuredValueKind.String, 0, 0, 0, 5, 5, 0, 0, 0),
            }, new uint[] { 1 }, 0, "empty"u8.ToArray())));
        _outputs = new(context.Engine);
        if (context.Input.CursorMode != InputCursorMode.Unlocked)
            throw new System.InvalidOperationException("Packaged cursor mode did not reach C# composition.");
        IInputService input = context.Engine.Input;
        ProductInputMapping initialMapping = context.Input.PhysicalMappings.Span[0];
        JsonPersistenceChecks.Run(context.Engine);
        SpatialResidencyChecks.Run(context.Engine);
        CharacterMeshChecks.Run(context.Engine);
        AddressableInventoryStacksExercise.Run();
        ProductInputMapping replacement = initialMapping with { Keyboard = KeyboardControl.KeyF };
        if (input.ReplacePhysicalMappings([replacement]) != InputMappingReplacementOutcome.Staged)
            throw new InvalidOperationException("Packaged input replacement did not stage.");
        if (input.ReplacePhysicalMappings([replacement, replacement]) != InputMappingReplacementOutcome.InvalidMappings)
            throw new InvalidOperationException("Duplicate mapping IDs did not return a recoverable outcome.");
        if (initialMapping.Keyboard != KeyboardControl.KeyE)
            throw new InvalidOperationException("Runtime replacement mutated the initial composition snapshot.");
    }
    public void Start() { }
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update) { _outputs.Tick(); return ProductUpdateResult.None; }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { _ui.Dispose(); }
}
EOF
cp "$repo_root/scripts/fixtures/JsonPersistenceChecks.cs" "$consumer/JsonPersistenceChecks.cs"
cp "$repo_root/scripts/fixtures/SpatialResidencyChecks.cs" "$consumer/SpatialResidencyChecks.cs"
cp "$repo_root/scripts/fixtures/CharacterMeshChecks.cs" "$consumer/CharacterMeshChecks.cs"
cp "$repo_root/scripts/fixtures/RenderOutputChecks.cs" "$consumer/RenderOutputChecks.cs"
cp "$repo_root/fixtures/render/assets/kenney-retro-character/character-medium.glb" "$consumer/content/animated.glb"
cp "$repo_root/fixtures/voxel-conversion/kenney-wall-a.glb" "$consumer/content/static.glb"
mkdir -p "$consumer/content/Textures"
cp "$repo_root/fixtures/csharp-nativeaot-trial/content/trial.png" "$consumer/content/Textures/wall_lines.png"
cp "$repo_root/fixtures/csharp-nativeaot-trial/content/trial.png" "$consumer/content/Textures/concrete.png"
cp "$repo_root/csharp/Rusty.Engine.Mechanics.Example/AddressableInventoryStacksExercise.cs" "$consumer/AddressableInventoryStacksExercise.cs"
cat > "$consumer/product-ui/main.js" <<'EOF'
export function mountProductUi(root, context) {
    root.dataset.fixture = 'ready';
    const unsubscribe = context.projection.subscribe((projection) => {
        if (projection?.contract !== 'pair.ui.v1') return;
        if (projection.value.empty !== '') throw new Error('Empty UI string changed in transport');
        root.dataset.emptyString = 'preserved';
    });
    return { dispose: unsubscribe };
}
EOF
printf 'pair-only content\n' > "$consumer/content/trial.txt"

consumer_home="$work/dotnet-home"
consumer_packages="$work/nuget-packages"
mkdir -p "$consumer_home" "$consumer_packages"
(
    cd "$consumer"
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet restore PairConsumer.csproj --configfile NuGet.Config
    DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet msbuild PairConsumer.csproj -t:StageRustyEngineCoreClrProduct -p:RustyEngineProductPort=0
)
staged=$(cd "$consumer" && DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
    dotnet msbuild PairConsumer.csproj -getProperty:RustyEngineStagedProductDirectory | tail -n 1)
[[ -f "$staged/product.json" && -f "$staged/coreclr/Rusty.Engine.Product.dll" ]] || {
    echo "RUSTY_ENGINE_PAIR_TEST_STAGE: package-only consumer did not stage a CoreCLR product" >&2
    exit 1
}
if rg -F -q "$repo_root" "$consumer" "$consumer_packages"; then
    echo "RUSTY_ENGINE_PAIR_TEST_SOURCE_LEAK: clean consumer acquired an Engine checkout path" >&2
    exit 1
fi
if find "$consumer" -type f \( -name 'NativeProduct.cs' -o -name 'NativeProduct.csproj' -o -name 'rusty_engine.h' \) | grep -q .; then
    echo "RUSTY_ENGINE_PAIR_TEST_PROVIDER_INPUT: clean consumer acquired provider interop or generated inputs" >&2
    exit 1
fi

host_log="$work/runtime-host.log"
if "$runtime/bin/rusty" dev --help > "$work/rusty-dev-help.log" 2>&1; then
    echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted rusty dev help unexpectedly started a session" >&2
    exit 1
fi
grep -F 'usage: rusty dev --project' "$work/rusty-dev-help.log" >/dev/null || {
    echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted runtime pack did not expose rusty dev" >&2
    exit 1
}
loaders=(coreclr)
if [[ "$run_aot" == --aot ]]; then
    loaders+=(nativeaot)
fi
for loader in "${loaders[@]}"; do
if [[ "$loader" == nativeaot ]]; then
    (cd "$consumer" && DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet msbuild PairConsumer.csproj -t:VerifyRustyEngineAot -p:RustyEngineProductPort=0)
fi
for reopen in 0 1; do
output_dir="$work/output-$loader-$reopen"
mkdir -p "$output_dir"
host_log="$work/runtime-host-$loader-$reopen.log"
control_fifo="$work/control-$loader-$reopen"
mkfifo "$control_fifo"
exec 9<>"$control_fifo"
control_open=1
env -u CARGO -u CARGO_HOME -u RUSTUP_HOME RUSTY_OUTPUT_TEST_DIR="$output_dir" RUSTY_OUTPUT_REOPEN="$reopen" \
    "$runtime/bin/rusty-product-host" --headless --supervised --runtime-instance-id "$$" --product "$staged" --loader "$loader" --persistence-root "$work/persistence-$loader" < "$control_fifo" 9>&- > "$host_log" 2>&1 &
host_pid=$!
origin=""
for _ in $(seq 1 40); do
    origin=$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$host_log" | head -n 1)
    if [[ -n "$origin" ]] && curl --fail --silent "$origin/" >/dev/null; then
        break
    fi
    sleep 0.25
done
[[ -n "$origin" ]] || { cat "$host_log" >&2; echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted runtime pack did not launch the CoreCLR product" >&2; exit 1; }
curl --fail --silent "$origin/product-bootstrap.json" | jq -e '.product.id == "fixture.release-pair" and .ui.entry == "product-ui/main.js" and .input.cursorMode == "unlocked"' >/dev/null \
    || { echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted runtime did not serve the staged Product" >&2; exit 1; }

[[ -s "$work/persistence-$loader/json-roundtrip/journey" ]] || {
    echo "JSON fixture did not write real persistent state" >&2; exit 1;
}
[[ ! -e "$work/persistence-$loader/json-roundtrip/discarded" ]] || {
    echo "JSON fixture did not remove deleted persistent state" >&2; exit 1;
}
for _ in $(seq 1 240); do
    [[ -f "$output_dir/complete" ]] && break
    kill -0 "$host_pid" 2>/dev/null || break
    sleep 0.25
done
if [[ ! -f "$output_dir/complete" ]]; then
    cat "$host_log" >&2
    curl --silent --max-time 5 -H 'Content-Type: application/json' --data '{}' \
        "$origin/__rusty/product/runtime/diagnostics/read" >&2 || true
    echo "Packaged renderer outputs did not complete" >&2
    exit 1
fi
python3 "$repo_root/scripts/fixtures/verify-render-outputs.py" "$output_dir"
exec 9>&-
control_open=0
wait "$host_pid"
host_pid=""
echo "Packaged $loader output checks passed (reopen=$reopen)"
if [[ "$reopen" == 0 ]]; then
    cp "$output_dir/"*.glb "$consumer/content/"
    (cd "$consumer" && DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
        dotnet msbuild PairConsumer.csproj -t:StageRustyEngineCoreClrProduct -p:RustyEngineProductPort=0)
    if [[ "$loader" == nativeaot ]]; then
        (cd "$consumer" && DOTNET_CLI_HOME="$consumer_home" NUGET_PACKAGES="$consumer_packages" \
            dotnet msbuild PairConsumer.csproj -t:VerifyRustyEngineAot -p:RustyEngineProductPort=0)
    fi
fi
done
echo "Packaged $loader inventory, input, persistence, residency, capture, and export checks passed"
done

tampered="$work/tampered-pair"
cp -a "$pair_root" "$tampered"
printf '\n' >> "$tampered/runtime-pack/runtime-manifest.json"
if "$tampered/verify-pair.sh" --directory "$tampered" > "$work/tamper.log" 2>&1; then
    echo "RUSTY_ENGINE_PAIR_TEST_TAMPER: verifier accepted a modified runtime manifest" >&2
    exit 1
fi
grep -F 'RUSTY_ENGINE_PAIR_PAYLOAD' "$work/tamper.log" >/dev/null || {
    cat "$work/tamper.log" >&2
    echo "RUSTY_ENGINE_PAIR_TEST_TAMPER: verifier did not report a pair payload failure" >&2
    exit 1
}

echo 'csharp release pair clean-consumer proof passed'
