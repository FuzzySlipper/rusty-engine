#!/usr/bin/env bash
set -euo pipefail

# Emit one RUSTY_PERF JSON record per independently attributable layer. These
# are local regression baselines, not universal pass/fail thresholds across
# different CPUs, browsers, GPUs, or software rasterizers.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-performance.XXXXXX)"
ITERATIONS="${RUSTY_PERF_ITERATIONS:-50}"
RENDER_PORT="${RUSTY_PERF_RENDER_PORT:-$((4200 + $$ % 1000))}"
BROWSER_RUNTIME_ROOT="${RUSTY_PERF_RUNTIME_PACK:-$REPO_ROOT/target/runtime-pack/linux-x64}"
SDK_VERSION="0.1.0-crossover.$$.${RANDOM}"
CROSSOVER_FIXTURE="$REPO_ROOT/fixtures/csharp-crossover-performance/CsharpCrossoverPerformance.csproj"
export RUSTY_ENGINE_FIXTURE_SDK_VERSION="$SDK_VERSION"
export RUSTY_PERF_PRODUCT_CONFIGURATION=Release

cleanup() {
  rm -rf -- "$PROBE_ROOT"
}
trap cleanup EXIT

cd "$REPO_ROOT"

if [[ ! -d "$BROWSER_RUNTIME_ROOT/share/browser" ]]; then
  echo "run-performance-regression: runtime browser shell is missing from $BROWSER_RUNTIME_ROOT; set RUSTY_PERF_RUNTIME_PACK to an existing runtime pack" >&2
  exit 2
fi

cargo test -p csharp-engine-services \
  performance_probe_appearance_call_stage \
  --release -- --ignored --nocapture

dotnet run \
  --project "$REPO_ROOT/fixtures/csharp-performance-probe/CsharpPerformanceProbe.csproj" \
  --configuration Release -- 10000

"$REPO_ROOT/scripts/pack-csharp-sdk.sh" "$SDK_VERSION" "$PROBE_ROOT/sdk-feed" >/dev/null
dotnet restore "$CROSSOVER_FIXTURE" \
  --source "$PROBE_ROOT/sdk-feed" \
  -p:RustyEngineFixtureSdkVersion="$SDK_VERSION"
dotnet msbuild "$CROSSOVER_FIXTURE" \
  -nologo \
  -verbosity:quiet \
  -t:VerifyRustyEngineAot \
  -p:Configuration=Release \
  -p:RustyEngineFixtureSdkVersion="$SDK_VERSION"

PRODUCT_DIRECTORY="$(dotnet msbuild "$CROSSOVER_FIXTURE" \
  -nologo \
  -verbosity:quiet \
  -getProperty:RustyEngineStagedProductDirectory \
  -p:Configuration=Release \
  -p:RustyEngineFixtureSdkVersion="$SDK_VERSION" | tail -n 1)"
if [[ ! -f "$PRODUCT_DIRECTORY/product.json" ]]; then
  echo "run-performance-regression: canonical crossover fixture did not stage product.json at $PRODUCT_DIRECTORY" >&2
  exit 1
fi

# Reuse the already-built browser shell without producing a runtime-pack
# artifact. The current source host is copied into this disposable layout so
# its executable-relative browser lookup remains the canonical host path.
HOST_LAYOUT="$PROBE_ROOT/runtime"
install -d "$HOST_LAYOUT/bin" "$HOST_LAYOUT/share"
cp -a "$BROWSER_RUNTIME_ROOT/share/browser" "$HOST_LAYOUT/share/browser"
cargo build -p csharp-product-runtime --bin rusty-product-host --release --locked
install -m 755 target/release/rusty-product-host "$HOST_LAYOUT/bin/rusty-product-host"

for loader in coreclr nativeaot; do
  "$HOST_LAYOUT/bin/rusty-product-host" \
    --product "$PRODUCT_DIRECTORY" \
    --loader "$loader" \
    --persistence-root "$PROBE_ROOT/$loader-persistence" \
    --content-store-root "$PROBE_ROOT/$loader-content-store" \
    --performance-probe "$ITERATIONS"
done

if [[ "${RUSTY_PERF_SKIP_BROWSER:-0}" != "1" ]]; then
  PLAYWRIGHT_RENDER_PORT="$RENDER_PORT" pnpm --dir "$REPO_ROOT/render" exec playwright test \
    browser/renderer-performance.browser.spec.ts \
    --config playwright.config.ts \
    --reporter=line
fi
