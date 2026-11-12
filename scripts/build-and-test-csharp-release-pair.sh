#!/usr/bin/env bash
set -euo pipefail

# Build this checkout's C# SDK/runtime pair and exercise it with a packaged
# consumer: the pair workflow's build and test steps, and the routed `pair`
# lane (`den-tool build run --lane pair`), whose retained output directory
# DEN_BUILD_OUTPUT_DIR is the default when no output directory is given.

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
if (($# > 1)); then
    echo "usage: scripts/build-and-test-csharp-release-pair.sh [<new-output-directory>]" >&2
    exit 2
fi
output=${1:-${DEN_BUILD_OUTPUT_DIR:?give a new output directory}/pair}

cd "$repo_root"
pnpm --dir render install --frozen-lockfile
./scripts/build-csharp-release-pair.sh --output "$output"
./scripts/test-csharp-release-pair.sh "$output"/*.tar.gz
printf 'Pair: %s\n' "$output"
