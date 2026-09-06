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

echo "generated C# SDK package/CoreCLR smoke and NativeAOT fidelity exercise passed"
