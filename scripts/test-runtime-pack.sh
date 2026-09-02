#!/usr/bin/env bash
set -euo pipefail

# Focused V1 Product-bundle proof: one moved Engine runtime pack starts the
# same logical fixture through both loaders. Product UI/content changes do not
# rebuild or alter the Engine-owned runtime artifact.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d -t rusty-engine-runtime-pack.XXXXXX)"
cleanup() {
  local result=$?
  if ((result == 0)); then
    rm -rf -- "$WORK"
  else
    echo "runtime-pack proof failed; retained diagnostics at $WORK" >&2
  fi
  return "$result"
}
trap cleanup EXIT

PACK_BUILD="$WORK/pack-build"
PACK="$WORK/pack"
PRODUCTS="$WORK/products"

"$REPO_ROOT/scripts/build-runtime-pack.sh" --output "$PACK_BUILD"
cp -a "$PACK_BUILD" "$PACK"
install -d "$PRODUCTS/native" "$PRODUCTS/coreclr"

dotnet publish "$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj" \
  --configuration Release --runtime linux-x64 --output "$PRODUCTS/native/native"
dotnet build "$REPO_ROOT/fixtures/csharp-debug-execution-context/CsharpDebugExecutionContext.csproj" \
  --configuration Debug --output "$PRODUCTS/coreclr/coreclr"

stage_product_files() {
  local destination="$1"
  install -d "$destination/ui" "$destination/content"
  cp -a "$REPO_ROOT/fixtures/csharp-nativeaot-trial/product-ui/." "$destination/ui/"
  cp -a "$REPO_ROOT/fixtures/csharp-nativeaot-trial/content/." "$destination/content/"
}
stage_product_files "$PRODUCTS/native"
stage_product_files "$PRODUCTS/coreclr"

cat > "$PRODUCTS/native/product.json" <<'JSON'
{"artifact":"rusty.product.bundle","schemaVersion":1,"product":{"id":"fixture.runtime","title":"Runtime Pack Fixture"},"nativeAot":{"module":"native/CsharpNativeAotTrial.so"},"ui":{"root":"ui","entry":"main.js","assets":"assets"},"content":{"root":"content"},"lifecycle":{"mode":"demand"},"input":{"intents":[{"id":"runtime.exercise","value":"payload:runtime.exercise.payload"},{"id":"runtime.exercise.move","value":"digital"}],"mappings":[{"id":"runtime.exercise.move","intent":"runtime.exercise.move","trigger":"key:key-w:held"}]},"server":{"port":0,"liveDebug":false}}
JSON
cat > "$PRODUCTS/coreclr/product.json" <<'JSON'
{"artifact":"rusty.product.bundle","schemaVersion":1,"product":{"id":"fixture.runtime","title":"Runtime Pack Fixture"},"coreclr":{"assembly":"coreclr/CsharpDebugExecutionContext.dll","runtimeconfig":"coreclr/CsharpDebugExecutionContext.runtimeconfig.json"},"ui":{"root":"ui","entry":"main.js","assets":"assets"},"content":{"root":"content"},"lifecycle":{"mode":"demand"},"input":{"intents":[{"id":"runtime.exercise","value":"payload:runtime.exercise.payload"},{"id":"runtime.exercise.move","value":"digital"}],"mappings":[{"id":"runtime.exercise.move","intent":"runtime.exercise.move","trigger":"key:key-w:held"}]},"server":{"port":0,"liveDebug":false}}
JSON

unset CARGO CARGO_HOME RUSTUP_HOME
cd "$WORK"
"$PACK/bin/rusty-product-host" --identity | grep -F 'rusty.product.runtime-identity' >/dev/null
"$PACK/bin/rusty-product-host" --version | grep -F 'rusty-product-host' >/dev/null
test -x "$PACK/bin/rusty"
if "$PACK/bin/rusty" dev --help >"$WORK/rusty-dev-help.log" 2>&1; then
  echo 'rusty dev --help unexpectedly started a development session' >&2
  exit 1
fi
grep -F 'usage: rusty dev --project' "$WORK/rusty-dev-help.log" >/dev/null

RUNTIME_HASH_BEFORE="$(find "$PACK" -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}')"

start_and_assert() {
  local loader="$1"
  local product="$2"
  shift 2
  local log="$WORK/$loader-host.log"
  "$PACK/bin/rusty-product-host" --product "$product" --loader "$loader" "$@" > "$log" 2>&1 &
  local pid=$!
  local ready=false
  for _ in $(seq 1 40); do
    if grep -q "product host listening at http://" "$log"; then
      local origin
      origin="$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$log" | head -n 1)"
      if curl --fail --silent --show-error "$origin/product-bootstrap.json" | grep -F 'product-ui/main.js' >/dev/null \
        && curl --fail --silent --show-error "$origin/product-ui/main.js" | grep -F 'fixture-product-ui-marker' >/dev/null \
        && [[ "$(curl --silent --output /dev/null --write-out '%{http_code}' "$origin/content/trial.txt")" == 404 ]]; then
        ready=true
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        break
      fi
    fi
    sleep 0.25
  done
  if [[ "$ready" != true ]]; then
    cat "$log" >&2
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    exit 1
  fi
}

start_and_assert nativeaot "$PRODUCTS/native" \
  --persistence-root "$PRODUCTS/native/persistence" \
  --content-store-root "$PRODUCTS/native/content-store"
start_and_assert coreclr "$PRODUCTS/coreclr"

# Alter only loose Product bytes; the moved runtime pack remains bit-for-bit
# unchanged and it remains the owner of every Engine browser-runtime asset.
printf '\n// staged Product-only UI revision\n' >> "$PRODUCTS/native/ui/main.js"
printf '\nstaged Product-only content revision\n' >> "$PRODUCTS/native/content/trial.txt"
RUNTIME_HASH_AFTER="$(find "$PACK" -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}')"
[[ "$RUNTIME_HASH_BEFORE" == "$RUNTIME_HASH_AFTER" ]]

echo 'moved runtime pack launched CoreCLR and NativeAOT Product V1 bundles; UI was staged and content remained non-static'
