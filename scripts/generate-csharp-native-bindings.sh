#!/usr/bin/env bash
set -euo pipefail

# Generate the trusted NativeAOT ABI from the one Rust source of truth. The
# cbindgen and the managed ClangSharp parser are pinned here and in the
# BindingGenerator project; generated source is written only to ignored local
# build paths.

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
crate_dir="$repo_root/rust/crates/csharp-engine-abi"
output_dir=${1:-"$repo_root/fixtures/csharp-nativeaot-trial/obj/Generated"}
inputs_dir=${2:-"$output_dir/GeneratedInputs"}
identity_rust="$crate_dir/src/generated_abi_identity.rs"
if [[ "$output_dir" != /* ]]; then
    output_dir="$repo_root/$output_dir"
fi
if [[ "$inputs_dir" != /* ]]; then
    inputs_dir="$repo_root/$inputs_dir"
fi

mkdir -p "$output_dir"

header="$output_dir/rusty_engine.h"
contracts="$output_dir/EngineContracts.g.cs"
values="$output_dir/EngineValues.g.cs"
outputs=(
    "$header"
    "$contracts"
    "$values"
    "$inputs_dir/Interop.g.cs"
    "$inputs_dir/EngineServiceImplementations.g.cs"
    "$inputs_dir/AbiIdentity.g.cs"
    "$identity_rust"
)

# Skip the whole pipeline when its declarations, generator, configuration,
# tools, and previous outputs are unchanged. The stamp records the input
# fingerprint plus output hashes, so a missing or edited output regenerates.
generator_dir="$repo_root/csharp/Rusty.Engine.BindingGenerator"
stamp="$output_dir/.generation-stamp"
input_fingerprint() {
    {
        find "$crate_dir/src" -name '*.rs' ! -path "$identity_rust" -print0
        printf '%s\0' "$crate_dir/Cargo.toml" "$crate_dir/cbindgen.toml" "$script_dir/$(basename -- "${BASH_SOURCE[0]}")"
        find "$generator_dir" \( -name bin -o -name obj \) -prune -o -type f -print0
    } | LC_ALL=C sort -z | xargs -0 sha256sum
    clang --version 2>&1 || true
    dotnet --version 2>&1 || true
    printf 'output %s\ninputs %s\n' "$output_dir" "$inputs_dir"
}
output_hashes() {
    sha256sum -- "${outputs[@]}" 2>/dev/null
}
fingerprint=$(input_fingerprint | sha256sum)
if [[ -f "$stamp" ]] && [[ "$(cat "$stamp")" == "$fingerprint"$'\n'"$(output_hashes)" ]]; then
    exit 0
fi
rm -f -- "$stamp"

cbindgen_version=0.29.4
cbindgen_root="$repo_root/target/cbindgen-$cbindgen_version"
cbindgen_bin="$cbindgen_root/bin/cbindgen"
if [[ ! -x "$cbindgen_bin" ]]; then
    cargo install cbindgen --version "$cbindgen_version" --locked --root "$cbindgen_root"
fi

if ! command -v clang >/dev/null 2>&1; then
    echo "generate-csharp-native-bindings: clang is required by BindingGenerator's ClangSharp parser" >&2
    exit 1
fi
clang_resource_dir=$(clang -print-resource-dir)

# Older runs emitted this ignored raw binding file. It is no longer part of the
# generated surface, so remove it when reusing an output directory rather than
# leaving stale source beside current bindings.
rm -f -- "$output_dir/NativeBindings.g.cs"

(
    cd "$crate_dir"
    "$cbindgen_bin" \
        --config cbindgen.toml \
        --crate csharp-engine-abi \
        --output "$header"
)

dotnet restore "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj"

dotnet run \
    --project "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" \
    --no-restore \
    -- "$header" "$contracts" "$values" "$inputs_dir" "$clang_resource_dir" "$identity_rust"

printf '%s\n%s' "$fingerprint" "$(output_hashes)" > "$stamp"
