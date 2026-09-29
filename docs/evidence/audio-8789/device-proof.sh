#!/usr/bin/env bash
# Plays the Engine audio-containers fixture on the runtime's output device and
# records what reached the sound server. Usage:
#   device-proof.sh <runtime-layout> <staged-product> <work-dir> <commands...>
# The runtime plays into a PulseAudio null sink (PULSE_SINK) whose monitor is
# recorded, so nothing reaches speakers. Each extra argument is one live-debug
# command line, run in order; `sleep:<seconds>` waits and
# `catalog` saves the debug catalog; `claim:<intent>:<contract>:<json>`
# submits a product-payload intent and `physical:<json fact>` a physical input; `post:<route>` posts the current binding to a runtime
# control route; `script:<python file>` runs a driver against the host. HOST_ARGS adds
# host arguments such as --persistence-root.
set -euo pipefail
RUNTIME="$1" PRODUCT="$2" WORK="$3"
shift 3
SINK=rusty_audio_8789
mkdir -p "$WORK"
parec --device="$SINK.monitor" --file-format=wav --format=s16le --rate=48000 --channels=2 \
  "$WORK/device.wav" &
RECORDER=$!
RECORD_START="$(date +%s.%N)"
# Seconds since the recording started, to match steps to recording windows.
elapsed() { echo "$(date +%s.%N) - $RECORD_START" | bc | xargs printf '%.2f'; }
# A packaged CoreCLR host is supervised and stops when stdin closes; the
# script holds a FIFO open as that stdin and closes it to shut down.
mkfifo "$WORK/host.stdin"
# AUDIO_OUTPUT= (empty) leaves audio to the browser for comparison.
AUDIO_OUTPUT="${AUDIO_OUTPUT-device}"
env ${AUDIO_OUTPUT:+RUSTY_AUDIO_OUTPUT=$AUDIO_OUTPUT} PULSE_SINK="$SINK" \
  DOTNET_ROOT="${DOTNET_ROOT:-/home/agent/.dotnet}" \
  "$RUNTIME/bin/rusty-product-host" --product "$PRODUCT" --loader coreclr ${HOST_ARGS:-} \
  <"$WORK/host.stdin" >"$WORK/host.log" 2>&1 &
HOST=$!
exec 4>"$WORK/host.stdin"
cleanup() {
  [[ -n "${SSE:-}" ]] && kill "$SSE" 2>/dev/null
  exec 4>&-
  timeout 20 tail --pid="$HOST" -f /dev/null || kill "$HOST" 2>/dev/null
  sleep 0.3
  kill "$RECORDER" 2>/dev/null || true
  wait "$RECORDER" 2>/dev/null || true
  rm -f "$WORK/host.stdin"
}
trap cleanup EXIT
for _ in $(seq 1 120); do
  grep -q "listening at http://" "$WORK/host.log" && break
  sleep 0.25
done
ORIGIN="$(sed -n 's/.*listening at \(http:\/\/[^ ]*\).*/\1/p' "$WORK/host.log" | head -n 1)"
[[ -n "$ORIGIN" ]] || { cat "$WORK/host.log" >&2; exit 1; }
# The first attachment starts the product, as a browser would.
curl --silent --no-buffer -H "Accept: text/event-stream" -H "Origin: $ORIGIN" "$ORIGIN/__rusty/product/runtime/outputs/fresh" \
  >"$WORK/outputs.sse" &
SSE=$!
# Commands start once the product has published its first binding.
for _ in $(seq 1 240); do
  grep -qE 'kind\\?":\\?"binding' "$WORK/outputs.sse" 2>/dev/null && break
  sleep 0.25
done
for line in "$@"; do
  if [[ "$line" == catalog ]]; then
    curl --silent -H "Origin: $ORIGIN" "$ORIGIN/__rusty/product/runtime/debug/catalog" >"$WORK/catalog.json"
    continue
  fi
  if [[ "$line" == claim:* || "$line" == physical:* ]]; then
    # claim:<intent>:<contract>:<json data> submits one product-payload
    # intent, and physical:<json fact> one physical input fact (for example
    # {"kind":"pointer-button","button":"primary","edge":"pressed"}), each under
    # the latest published binding, as the browser shell does.
    body="$(python3 - "$WORK/outputs.sse" "$line" <<'PY'
import json, re, sys
# Large batches arrive as fragments carrying escaped JSON.
stream = open(sys.argv[1]).read().replace('\\"', '"')
runtime = json.loads(re.findall(r'\{"kind":"binding","runtime":(\{[^}]*\})', stream)[-1])
# Input results advance the sequence after the binding published it.
sequence = re.findall(r'"nextInputSequence":"(\d+)"', stream)[-1]
event = {"runtime": runtime, "sequence": sequence, "context": "gameplay.default"}
line = sys.argv[2]
if line.startswith("claim:"):
    _, intent, contract, data = line.split(":", 3)
    event.update(intent=intent, value={"kind": "product-payload", "contract": contract, "data": json.loads(data)})
else:
    event["fact"] = json.loads(line[len("physical:"):])
print(json.dumps({"batch": [event]}))
PY
)"
    echo "> [$(elapsed)s] $line" >>"$WORK/live-debug.log"
    curl --silent -H "Origin: $ORIGIN" -H "Content-Type: application/json" \
      --data "$body" "$ORIGIN/__rusty/product/runtime/input" >>"$WORK/live-debug.log"
    echo >>"$WORK/live-debug.log"
    sleep 0.2
    continue
  fi
  if [[ "$line" == post:* ]]; then
    echo "> [$(elapsed)s] $line" >>"$WORK/live-debug.log"
    runtime="$(grep -o '{"kind":"binding","runtime":{[^}]*}' "$WORK/outputs.sse" | tail -n 1 \
      | sed 's/.*"runtime"://')"
    curl --silent -H "Origin: $ORIGIN" -H "Content-Type: application/json" \
      --data "{\"runtime\":$runtime}" "$ORIGIN/__rusty/product/runtime/${line#post:}" \
      | head -c 300 >>"$WORK/live-debug.log"
    echo >>"$WORK/live-debug.log"
    continue
  fi
  if [[ "$line" == script:* ]]; then
    # script:<python file> drives the product itself; it receives the origin,
    # the output stream capture and the live-debug client.
    echo "> [$(elapsed)s] $line" >>"$WORK/live-debug.log"
    python3 "${line#script:}" "$ORIGIN" "$WORK/outputs.sse" "$RUNTIME/bin/rusty-live-debug" >>"$WORK/live-debug.log" 2>&1
    continue
  fi
  if [[ "$line" == sleep:* ]]; then
    sleep "${line#sleep:}"
    continue
  fi
  echo "> [$(elapsed)s] $line" >>"$WORK/live-debug.log"
  "$RUNTIME/bin/rusty-live-debug" --origin "$ORIGIN" --command "$line" >>"$WORK/live-debug.log" 2>&1
  echo >>"$WORK/live-debug.log"
done
