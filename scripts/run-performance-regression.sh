#!/usr/bin/env bash
set -euo pipefail

# Emit one RUSTY_PERF JSON record per independently attributable layer. These
# are local regression baselines, not universal pass/fail thresholds across
# different CPUs, browsers, GPUs, or software rasterizers.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-performance.XXXXXX)"
ITERATIONS="${RUSTY_PERF_ITERATIONS:-50}"
RENDER_PORT="${RUSTY_PERF_RENDER_PORT:-$((4200 + $$ % 1000))}"

cleanup() {
  rm -rf -- "$PROBE_ROOT"
}
trap cleanup EXIT

cd "$REPO_ROOT"

cargo test -p csharp-engine-services \
  performance_probe_appearance_call_stage \
  --release -- --ignored --nocapture

dotnet run \
  --project "$REPO_ROOT/fixtures/csharp-performance-probe/CsharpPerformanceProbe.csproj" \
  --configuration Release -- 10000

dotnet restore \
  "$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj" \
  --runtime linux-x64
dotnet publish \
  "$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj" \
  --configuration Release \
  --runtime linux-x64 \
  --no-restore \
  --output "$PROBE_ROOT/product"

cargo run -p csharp-product-runtime \
  --bin csharp-product-runtime \
  --release \
  --locked -- \
  --library "$PROBE_ROOT/product/CsharpNativeAotTrial.so" \
  --bundle-dir "$REPO_ROOT/fixtures/csharp-nativeaot-trial/browser" \
  --content-dir "$REPO_ROOT/fixtures/csharp-nativeaot-trial/content" \
  --mode demand \
  --persistence-root "$PROBE_ROOT/persistence" \
  --content-store-root "$PROBE_ROOT/content-store" \
  --direct-intent runtime.exercise=payload:runtime.exercise.payload \
  --port 0 \
  --performance-probe "$ITERATIONS"

PLAYWRIGHT_RENDER_PORT="$RENDER_PORT" pnpm --dir "$REPO_ROOT/render" exec playwright test \
  browser/renderer-performance.browser.spec.ts \
  --config playwright.config.ts \
  --reporter=line
