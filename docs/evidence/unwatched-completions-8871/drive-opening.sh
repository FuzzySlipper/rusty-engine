#!/usr/bin/env bash
# #8871: Dagger's opening with no page attached, driven over HTTP only.
set -u
origin=http://127.0.0.1:4394
debug() { curl -s -X POST -H 'Content-Type: text/plain; charset=utf-8' --data "$1" "$origin/__rusty/product/runtime/debug/execute"; }
binding=$(debug engine.renderer.presentation | jq -c '.runtime')
echo "binding $binding"
body=$(jq -cn --argjson runtime "$binding" '{batch: [{runtime: $runtime, sequence: "1", context: "gameplay.default", intent: "dagger.ui", value: {kind: "product-payload", contract: "dagger.ui.action.v1", data: {action: "begin"}}}]}')
curl -s -X POST -H 'Content-Type: application/json' --data "$body" "$origin/__rusty/product/runtime/input"; echo
start=$(date +%s)
last=""
while true; do
  mode=$(debug playtest.observe | jq -r '.mode')
  now=$(( $(date +%s) - start ))
  if [ "$mode" != "$last" ]; then echo "t=${now}s mode=$mode step=$(debug engine.time | jq -r '.simulationStep')"; last=$mode; fi
  [ "$mode" = "Playing" ] && break
  [ $now -gt 330 ] && { echo "timeout"; break; }
  sleep 2
done
echo "drawn: $(debug engine.renderer.presentation | jq -c '{available, presentation}')"
