#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
fixture_dir="$repo_root/fixtures/csharp-binding-generator-lease"
output_dir="$fixture_dir/obj/Generated"

mkdir -p "$output_dir"
dotnet run --project "$repo_root/csharp/Rusty.Engine.BindingGenerator/Rusty.Engine.BindingGenerator.csproj" --no-restore -- \
    "$fixture_dir/lease-fixture.h" \
    "$output_dir/EngineContracts.g.cs" \
    "$output_dir/EngineValues.g.cs" \
    "$output_dir/GeneratedInputs" \
    "$(clang -print-resource-dir)"
dotnet restore "$fixture_dir/LeaseFixture.csproj"
dotnet run --project "$fixture_dir/LeaseFixture.csproj" --no-restore
