#!/usr/bin/env bash
set -euo pipefail

# Ordinary C# development consumes the immutable SDK package, stages the
# generated CoreCLR Product, and runs it through the Rust host. NativeAOT is a
# deliberate fidelity check; keep it opt-in so routine C# CI follows the same
# CoreCLR path as `rusty dev`.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  echo "usage: $(basename "$0") [--aot]" >&2
}

verify_aot=false
while (($#)); do
  case "$1" in
    --aot)
      verify_aot=true
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

package_arguments=(--coreclr-smoke)
if [[ "$verify_aot" == true ]]; then
  package_arguments+=(--aot)
fi
"$REPO_ROOT/scripts/test-csharp-sdk-package.sh" "${package_arguments[@]}"

if [[ "$verify_aot" != true ]]; then
  echo "generated C# SDK package, CoreCLR Product staging, and Rust host lifecycle smoke passed"
  exit 0
fi

# Retain the existing NativeAOT product-host exercise as an explicit fidelity
# probe. The package script above has already verified the generated AOT
# composition; this fixture continues to prove the direct native load path.
MANAGED_PROJECT="$REPO_ROOT/csharp/Rusty.Engine.Application.Example/Rusty.Engine.Application.Example.csproj"
NATIVE_AOT_PROJECT="$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj"
EXERCISE_ROOT="$(mktemp -d -t rusty-engine-csharp.XXXXXX)"
cleanup() {
  rm -rf -- "$EXERCISE_ROOT"
}
trap cleanup EXIT

dotnet restore "$NATIVE_AOT_PROJECT" --runtime linux-x64
dotnet restore "$MANAGED_PROJECT"
dotnet build "$MANAGED_PROJECT" --no-restore
dotnet publish "$NATIVE_AOT_PROJECT" \
  --configuration Release \
  --runtime linux-x64 \
  --no-restore \
  --output "$EXERCISE_ROOT/product"

cargo run --manifest-path "$REPO_ROOT/Cargo.toml" -p csharp-product-runtime --bin rusty-product-host --locked -- \
  --library "$EXERCISE_ROOT/product/CsharpNativeAotTrial.so" \
  --bundle-dir "$REPO_ROOT/fixtures/csharp-nativeaot-trial/browser" \
  --content-dir "$REPO_ROOT/fixtures/csharp-nativeaot-trial/content" \
  --mode demand \
  --persistence-root "$EXERCISE_ROOT/persistence" \
  --content-store-root "$EXERCISE_ROOT/content-store" \
  --direct-intent runtime.exercise=payload:runtime.exercise.payload \
  --port 0 \
  --exercise

echo "generated C# SDK package/CoreCLR smoke and NativeAOT fidelity exercise passed"
