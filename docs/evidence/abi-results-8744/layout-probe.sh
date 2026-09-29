#!/usr/bin/env bash
set -euo pipefail

# Prints the C (cbindgen header, compiled by clang) and Rust (repr(C)) layout
# of the Dynamics borrowed results, then diffs them. The generated C# structs
# come from the same clang AST, and the ABI fingerprint hashes these offsets.
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
header=${1:-"$repo_root/fixtures/csharp-nativeaot-trial/obj/Generated/rusty_engine.h"}
work=$(mktemp -d "${TMPDIR:-/tmp}/abi-8744-probe.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

types=(NativeDynamicsStepAndReadResult:bodies,bodies_len,generation,body_count,contact_count
       NativeDynamicsWorldResult:bodies,bodies_len,contacts,contacts_len,generation
       NativeDynamicsBodyFact:body,readout
       NativeDynamicsContact:environment,first,second,impulse,impulse_magnitude)

{
    printf '#include <stdio.h>\n#include <stddef.h>\n#include "%s"\nint main(void) {\n' "$header"
    for entry in "${types[@]}"; do
        type=${entry%%:*}
        printf '  printf("%s size=%%zu align=%%zu\\n", sizeof(%s), _Alignof(%s));\n' "$type" "$type" "$type"
        IFS=, read -ra fields <<<"${entry#*:}"
        for field in "${fields[@]}"; do
            printf '  printf("  %s=%%zu\\n", offsetof(%s, %s));\n' "$field" "$type" "$field"
        done
    done
    printf '  return 0;\n}\n'
} >"$work/probe.c"
clang -std=c11 -o "$work/probe" "$work/probe.c"
"$work/probe" >"$work/c.txt"

mkdir -p "$work/rust/src"
cat >"$work/rust/Cargo.toml" <<TOML
[package]
name = "abi-8744-probe"
version = "0.0.0"
edition = "2021"
[dependencies]
csharp-engine-abi = { path = "$repo_root/rust/crates/csharp-engine-abi" }
[workspace]
TOML
{
    printf 'use csharp_engine_abi::*;\nuse std::mem::{align_of, offset_of, size_of};\nfn main() {\n'
    for entry in "${types[@]}"; do
        type=${entry%%:*}
        printf '    println!("%s size={} align={}", size_of::<%s>(), align_of::<%s>());\n' "$type" "$type" "$type"
        IFS=, read -ra fields <<<"${entry#*:}"
        for field in "${fields[@]}"; do
            printf '    println!("  %s={}", offset_of!(%s, %s));\n' "$field" "$type" "$field"
        done
    done
    printf '}\n'
} >"$work/rust/src/main.rs"
CARGO_TARGET_DIR="$repo_root/target/abi-8744-probe" cargo run --quiet --manifest-path "$work/rust/Cargo.toml" >"$work/rust.txt"

cat "$work/c.txt"
if diff -u "$work/c.txt" "$work/rust.txt"; then
    echo "C and Rust layouts match"
fi
grep -o 'NativeStepAndReadDynamics { internal delegate\* unmanaged\[Cdecl\]<[^>]*>' \
    "$(dirname -- "$header")/GeneratedInputs/Interop.g.cs"
grep -o 'NativeReadDynamicsWorld { internal delegate\* unmanaged\[Cdecl\]<[^>]*>' \
    "$(dirname -- "$header")/GeneratedInputs/Interop.g.cs"
