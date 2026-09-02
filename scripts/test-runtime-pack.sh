#!/usr/bin/env bash
set -euo pipefail

# Focused proof for the packed host: move it away from this checkout, then
# start existing CoreCLR and NativeAOT fixtures without Cargo/source inputs.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d -t rusty-engine-runtime-pack.XXXXXX)"
cleanup() { rm -rf -- "$WORK"; }
trap cleanup EXIT

PACK_BUILD="$WORK/pack-build"
PACK="$WORK/pack"
FIXTURES="$WORK/fixtures"

"$REPO_ROOT/scripts/build-runtime-pack.sh" --output "$PACK_BUILD"
cp -a "$PACK_BUILD" "$PACK"
install -d "$FIXTURES/native" "$FIXTURES/coreclr" "$FIXTURES/content"

dotnet publish "$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj" \
  --configuration Release --runtime linux-x64 --output "$FIXTURES/native"
dotnet build "$REPO_ROOT/fixtures/csharp-debug-execution-context/CsharpDebugExecutionContext.csproj" \
  --configuration Debug --output "$FIXTURES/coreclr"
cp -a "$REPO_ROOT/fixtures/csharp-nativeaot-trial/content/." "$FIXTURES/content/"

printf '%s\n' '{"artifact":"rusty.product.runtime-launch","schemaVersion":1,"loader":"nativeaot"}' \
  > "$FIXTURES/native-launch.json"
printf '%s\n' '{"artifact":"rusty.product.runtime-launch","schemaVersion":1,"loader":"coreclr"}' \
  > "$FIXTURES/coreclr-launch.json"

unset CARGO CARGO_HOME RUSTUP_HOME
cd "$WORK"
"$PACK/bin/rusty-product-host" --identity | grep -F 'rusty.product.runtime-identity' >/dev/null
"$PACK/bin/rusty-product-host" --version | grep -F 'rusty-product-host' >/dev/null

"$PACK/bin/rusty-product-host" \
  --staged-launch "$FIXTURES/native-launch.json" \
  --library "$FIXTURES/native/CsharpNativeAotTrial.so" \
  --bundle-dir "$PACK/share/browser" \
  --content-dir "$FIXTURES/content" \
  --mode demand \
  --persistence-root "$FIXTURES/persistence" \
  --content-store-root "$FIXTURES/content-store" \
  --direct-intent runtime.exercise=payload:runtime.exercise.payload \
  --direct-intent runtime.exercise.move=digital \
  --physical-mapping runtime.exercise.move=runtime.exercise.move:key:key-w:held \
  --port 0 > "$WORK/native-host.log" 2>&1 &
NATIVE_HOST_PID=$!
NATIVE_READY=false
native_cleanup() {
  kill "$NATIVE_HOST_PID" 2>/dev/null || true
  wait "$NATIVE_HOST_PID" 2>/dev/null || true
}
trap 'native_cleanup; cleanup' EXIT
for _ in $(seq 1 40); do
  if grep -q 'NativeAOT product host listening at http://' "$WORK/native-host.log"; then
    NATIVE_ORIGIN="$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$WORK/native-host.log" | head -n 1)"
    if curl --fail --silent --show-error "$NATIVE_ORIGIN/" >/dev/null; then
      NATIVE_READY=true
      native_cleanup
      trap cleanup EXIT
      break
    fi
  fi
  sleep 0.25
done
if [[ "$NATIVE_READY" != true ]]; then
  cat "$WORK/native-host.log" >&2
  native_cleanup
  exit 1
fi

"$PACK/bin/rusty-product-host" \
  --staged-launch "$FIXTURES/coreclr-launch.json" \
  --library "$FIXTURES/coreclr/CsharpDebugExecutionContext.dll" \
  --runtimeconfig "$FIXTURES/coreclr/CsharpDebugExecutionContext.runtimeconfig.json" \
  --bundle-dir "$PACK/share/browser" \
  --content-dir "$FIXTURES/content" \
  --mode demand --port 0 > "$WORK/coreclr-host.log" 2>&1 &
CORECLR_HOST_PID=$!
CORECLR_READY=false
coreclr_cleanup() {
  kill "$CORECLR_HOST_PID" 2>/dev/null || true
  wait "$CORECLR_HOST_PID" 2>/dev/null || true
}
trap 'coreclr_cleanup; cleanup' EXIT
for _ in $(seq 1 40); do
  if grep -q 'CoreCLR product host listening at http://' "$WORK/coreclr-host.log"; then
    CORECLR_ORIGIN="$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$WORK/coreclr-host.log" | head -n 1)"
    if curl --fail --silent --show-error "$CORECLR_ORIGIN/" >/dev/null; then
      CORECLR_READY=true
      coreclr_cleanup
      trap cleanup EXIT
      break
    fi
  fi
  sleep 0.25
done
if [[ "$CORECLR_READY" != true ]]; then
  cat "$WORK/coreclr-host.log" >&2
  coreclr_cleanup
  exit 1
fi

echo 'moved runtime pack started NativeAOT and CoreCLR fixtures without Cargo/source inputs'
