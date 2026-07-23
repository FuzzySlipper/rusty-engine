#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$REPO_ROOT/ts/packages/game-runtime/native/game_bridge_napi.node"

cd "$REPO_ROOT"
cargo build -p game-bridge-napi

case "$(uname -s)" in
  Linux) ARTIFACT="$REPO_ROOT/target/debug/libgame_bridge_napi.so" ;;
  Darwin) ARTIFACT="$REPO_ROOT/target/debug/libgame_bridge_napi.dylib" ;;
  MINGW*|MSYS*|CYGWIN*) ARTIFACT="$REPO_ROOT/target/debug/game_bridge_napi.dll" ;;
  *) echo "unsupported native addon platform" >&2; exit 1 ;;
esac

test -f "$ARTIFACT"
mkdir -p "$(dirname "$DEST")"
cp "$ARTIFACT" "$DEST"
