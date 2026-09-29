#!/usr/bin/env bash
# Launch one topology against a staged Product directory.
#   RUSTY_PACK_BIN  runtime-pack bin directory (the prototype needs the
#                   host binary from 365ecf6a or later for `owned`)
#   RUSTY_PRODUCT   staged Product directory (contains product.json)
#   RUSTY_WORK      scratch directory for persistence roots
# Usage: start.sh worker|owned|inprocess
set -euo pipefail
: "${RUSTY_PACK_BIN:?}" "${RUSTY_PRODUCT:?}" "${RUSTY_WORK:?}"
mode=$1
roots=(--persistence-root "$RUSTY_WORK/p-$mode" --content-store-root "$RUSTY_WORK/cs-$mode")
rm -rf "$RUSTY_WORK/p-$mode" "$RUSTY_WORK/cs-$mode"
case $mode in
  # Current production topology: foreground shell relays for a CoreCLR worker.
  worker) exec "$RUSTY_PACK_BIN/rusty-product-host" --product "$RUSTY_PRODUCT" --loader coreclr "${roots[@]}" ;;
  # Prototype: supervisor binds; the runtime process serves the browser.
  # Creating $RUSTY_EXPERIMENT_REPLACE_FILE replaces the runtime.
  owned) export RUSTY_EXPERIMENT_REPLACE_FILE="$RUSTY_WORK/replace"
         exec "$RUSTY_PACK_BIN/rusty-product-host" --product "$RUSTY_PRODUCT" --loader coreclr --worker-owned-io "${roots[@]}" ;;
  # Reference: everything in one process (no signal isolation), port 40822.
  inprocess) exec "$RUSTY_PACK_BIN/rusty-product-host" --loader coreclr \
    --library "$RUSTY_PRODUCT/coreclr/Rusty.Engine.Product.dll" \
    --runtimeconfig "$RUSTY_PRODUCT/coreclr/Rusty.Engine.Product.runtimeconfig.json" \
    --bundle-dir "$RUSTY_WORK/host-bundle" --content-dir "$RUSTY_PRODUCT/content" \
    --mode realtime --port 40822 "${roots[@]}" ;;
  *) echo "usage: start.sh worker|owned|inprocess" >&2; exit 2 ;;
esac
