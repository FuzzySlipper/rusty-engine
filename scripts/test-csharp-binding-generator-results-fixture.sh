#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
fixture_dir="$repo_root/fixtures/csharp-binding-generator-results"
output_dir="$fixture_dir/obj/Generated"
invalid_output_dir=$(mktemp -d)
trap 'rm -rf "$invalid_output_dir"' EXIT

mkdir -p "$output_dir"
dotnet run --project "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" --no-restore -- \
    "$fixture_dir/result-fixture.h" \
    "$output_dir/EngineContracts.g.cs" \
    "$output_dir/EngineValues.g.cs" \
    "$output_dir/GeneratedInputs" \
    "$(clang -print-resource-dir)" \
    "$output_dir/generated_abi_identity.rs"
if dotnet run --project "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" --no-restore -- \
    "$fixture_dir/result-fixture-invalid-outer-borrow.h" \
    "$invalid_output_dir/EngineContracts.g.cs" \
    "$invalid_output_dir/EngineValues.g.cs" \
    "$invalid_output_dir/GeneratedInputs" \
    "$(clang -print-resource-dir)" \
    "$invalid_output_dir/generated_abi_identity.rs" >"$invalid_output_dir/rejected.txt" 2>&1; then
    echo "expected borrowed result metadata to be rejected" >&2
    exit 1
fi
rg -q "result metadata source .*not a supported fixed value" "$invalid_output_dir/rejected.txt"
if dotnet run --project "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" --no-restore -- \
    "$fixture_dir/result-fixture-invalid-borrowed-span.h" \
    "$invalid_output_dir/EngineContracts.g.cs" \
    "$invalid_output_dir/EngineValues.g.cs" \
    "$invalid_output_dir/GeneratedInputs" \
    "$(clang -print-resource-dir)" \
    "$invalid_output_dir/generated_abi_identity.rs" >"$invalid_output_dir/borrowed-span-rejected.txt" 2>&1; then
    echo "expected nested borrowed span pointer to be rejected" >&2
    exit 1
fi
rg -q "borrowed span element NativeInvalidBorrowedTag.unsupported_nested_pointer" "$invalid_output_dir/borrowed-span-rejected.txt"
dotnet restore "$fixture_dir/ResultFixture.csproj"
dotnet run --project "$fixture_dir/ResultFixture.csproj" --no-restore
