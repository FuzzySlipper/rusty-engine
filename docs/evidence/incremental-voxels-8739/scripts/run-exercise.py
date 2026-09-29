# Usage: run-exercise.py <rusty-product-host> <staged Product dir> <out.json>
# Runs the Voxel exercise under a headless browser until it reports, then
# records its facts line and the host's update telemetry.
import json, os, signal, subprocess, sys, time, urllib.request

host, product, out = sys.argv[1:4]
URL = "http://127.0.0.1:40851"


errors = []


def telemetry():
    request = urllib.request.Request(URL + "/__rusty/product/runtime/diagnostics/read", data=b"{}",
                                     headers={"Content-Type": "application/json"})
    read = json.loads(urllib.request.urlopen(request, timeout=5).read())
    for event in read.get("events", []):
        if event.get("severity") == "error" and event["message"] not in errors:
            errors.append(event["message"])
    return read["telemetry"]


process = subprocess.Popen([host, "--product", product, "--loader", "coreclr", "--headless"],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                           text=True, start_new_session=True)
os.set_blocking(process.stdout.fileno(), False)
lines, facts, verdict, last = [], None, None, None
deadline = time.time() + 120
while time.time() < deadline and verdict is None:
    time.sleep(0.5)
    try:
        chunk = process.stdout.read() or ""
    except BlockingIOError:
        chunk = ""
    for line in chunk.splitlines():
        lines.append(line)
        if line.startswith("VOXEL_FACTS "):
            facts = json.loads(line[len("VOXEL_FACTS "):])
        if line.startswith("VOXEL_EXERCISE_"):
            verdict = line
    try:
        last = telemetry()
    except Exception:
        pass
os.killpg(process.pid, signal.SIGINT)
process.wait(timeout=30)
attribution = (last or {}).get("updateAttribution") or {}
result = {
    "verdict": verdict,
    "facts": facts,
    "hostUpdates": {
        "samples": attribution.get("sampleCount"),
        "callbackUsP50": attribution.get("callbackDurationUsP50"),
        "callbackUsP95": attribution.get("callbackDurationUsP95"),
    },
    "errors": [error[:600] for error in errors],
    "log": [line for line in lines if "gcm" not in line][:40],
}
json.dump(result, open(out, "w"), indent=2)
print(json.dumps({key: result[key] for key in ("verdict", "facts", "hostUpdates")}, indent=2))
