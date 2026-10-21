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
# The generator's borrowed-result copies and refusals, against a hand-written table.
"$REPO_ROOT/scripts/test-csharp-binding-generator-results-fixture.sh"
# The fixtures that reference the Engine's C# projects rather than the package
# (the package's fixtures compile in the SDK smoke above) still compile.
while IFS= read -r fixture_project; do
  dotnet build "$REPO_ROOT/$fixture_project" --nologo --verbosity quiet
done < <(cd "$REPO_ROOT" && git grep -l 'csharp/Rusty.Engine/Rusty.Engine.csproj' -- 'fixtures/csharp-*/*.csproj')
# The SDK's example projects implement its service interfaces in fakes, so a
# new interface member must reach them too.
while IFS= read -r example_project; do
  dotnet build "$REPO_ROOT/$example_project" --nologo --verbosity quiet
done < <(cd "$REPO_ROOT" && git ls-files -- 'csharp/*.Example/*.csproj')
dotnet run --project "$REPO_ROOT/csharp/Rusty.Engine.Content.Example" --configuration Release
dotnet run --project "$REPO_ROOT/csharp/Rusty.Engine.Implicit.Example" --configuration Release

if [[ "$verify_aot" != true ]]; then
  echo "generated C# SDK package, CoreCLR Product staging, and Rust host lifecycle smoke passed"
  exit 0
fi

echo "generated C# SDK package/CoreCLR smoke and NativeAOT fidelity exercise passed"
