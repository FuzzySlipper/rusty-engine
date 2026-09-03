#!/usr/bin/env bash
set -euo pipefail

# Focused external-consumer proof for a published C# SDK/runtime pair.  The
# fixture lives entirely outside the checkout and restores Rusty.Engine only
# from the pair's embedded feed, so Cargo, generator inputs, and Engine browser
# files cannot be accidental product dependencies. Standard .NET platform
# reference packs may still resolve from NuGet on machines that do not bundle
# them with the installed SDK.

if [[ $# -ne 1 ]]; then
    echo "usage: scripts/test-csharp-release-pair.sh <pair.tar.gz>" >&2
    exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
archive=$1
"$script_dir/verify-csharp-release-pair.sh" --archive "$archive" >/dev/null

work=$(mktemp -d "${TMPDIR:-/tmp}/rusty-engine-pair-consumer.XXXXXX")
host_pid=""
cleanup() {
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
    <RustyEngineProductEntryType>PairConsumer.Product</RustyEngineProductEntryType>
    <RustyEngineProductId>fixture.release-pair</RustyEngineProductId>
    <RustyEngineProductTitle>Release pair fixture</RustyEngineProductTitle>
    <RustyEngineProductLifecycleMode>realtime</RustyEngineProductLifecycleMode>
    <RustyEngineProductFixedStepHz>60</RustyEngineProductFixedStepHz>
    <RustyEngineProductFixedStepMaxCatchUpSteps>4</RustyEngineProductFixedStepMaxCatchUpSteps>
  </PropertyGroup>
  <ItemGroup><PackageReference Include="Rusty.Engine" Version="$version" /></ItemGroup>
</Project>
EOF
cat > "$consumer/Product.cs" <<'EOF'
using Rusty.Engine;

namespace PairConsumer;

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
printf '// pair-only product UI\n' > "$consumer/product-ui/main.js"
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
env -u CARGO -u CARGO_HOME -u RUSTUP_HOME \
    "$runtime/bin/rusty-product-host" --product "$staged" --loader coreclr > "$host_log" 2>&1 &
host_pid=$!
origin=""
for _ in $(seq 1 40); do
    origin=$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$host_log" | head -n 1)
    if [[ -n "$origin" ]] && curl --fail --silent "$origin/" | grep -F 'Rusty Engine browser runtime connected' >/dev/null; then
        break
    fi
    sleep 0.25
done
[[ -n "$origin" ]] || { cat "$host_log" >&2; echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted runtime pack did not launch the CoreCLR product" >&2; exit 1; }
curl --fail --silent "$origin/product-bootstrap.json" | jq -e '.product.id == "fixture.release-pair" and .product.ui.entry == "product-ui/main.js"' >/dev/null \
    || { echo "RUSTY_ENGINE_PAIR_TEST_RUNTIME: extracted runtime did not serve the staged Product" >&2; exit 1; }

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
