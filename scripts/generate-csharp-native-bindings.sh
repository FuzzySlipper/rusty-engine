#!/usr/bin/env bash
set -euo pipefail

# Generate the trusted NativeAOT ABI from the one Rust source of truth. The
# tools are pinned here/at the repository tool manifest and are installed into
# ignored local build paths; no generated source is checked in.

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
crate_dir="$repo_root/rust/crates/csharp-product-runtime"
output_dir=${1:-"$repo_root/fixtures/csharp-nativeaot-trial/obj/Generated"}
if [[ "$output_dir" != /* ]]; then
    output_dir="$repo_root/$output_dir"
fi

mkdir -p "$output_dir"

cbindgen_version=0.29.4
cbindgen_root="$repo_root/target/cbindgen-$cbindgen_version"
cbindgen_bin="$cbindgen_root/bin/cbindgen"
if [[ ! -x "$cbindgen_bin" ]]; then
    cargo install cbindgen --version "$cbindgen_version" --locked --root "$cbindgen_root"
fi

if ! command -v clang >/dev/null 2>&1; then
    echo "generate-csharp-native-bindings: clang is required by ClangSharpPInvokeGenerator" >&2
    exit 1
fi

dotnet tool restore --tool-manifest "$repo_root/.config/dotnet-tools.json" --verbosity quiet

header="$output_dir/rusty_engine.h"
bindings="$output_dir/NativeBindings.g.cs"
facade="$output_dir/EngineApi.g.cs"

(
    cd "$crate_dir"
    "$cbindgen_bin" \
        --config cbindgen.toml \
        --crate csharp-product-runtime \
        --output "$header"
)

(
    cd "$repo_root"
    dotnet tool run \
        ClangSharpPInvokeGenerator \
        -f "$header" \
        -o "$bindings" \
        -n Rusty.Engine.Native \
        -x c \
        -rd "$(clang -print-resource-dir)" \
        -c codegen=latest \
        --generate helper-types \
        --generate generated-code=none \
        -e 'rusty_product_*'
)

python3 "$script_dir/generate-csharp-safe-facade.py" "$header" "$facade"
