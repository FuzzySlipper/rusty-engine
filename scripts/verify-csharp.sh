#!/usr/bin/env bash
set -euo pipefail

# Build from the generated ABI inputs in ignored obj directories, then publish
# one small NativeAOT product fixture. No generated C# artifacts are tracked.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANAGED_PROJECT="$REPO_ROOT/csharp/Rusty.Engine.Application.Example/Rusty.Engine.Application.Example.csproj"
NATIVE_AOT_PROJECT="$REPO_ROOT/fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj"

dotnet restore "$NATIVE_AOT_PROJECT" --runtime linux-x64
dotnet restore "$MANAGED_PROJECT"
dotnet build "$MANAGED_PROJECT" --no-restore
dotnet publish "$NATIVE_AOT_PROJECT" \
  --configuration Release \
  --runtime linux-x64 \
  --no-restore

echo "generated C# SDK build and NativeAOT fixture publish passed"
