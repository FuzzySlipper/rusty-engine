# Usage: bench.py <label> <rusty-product-host> <Product dir> <out.json>
import json, os, signal, subprocess, sys, time, urllib.request
label, host, product, out = sys.argv[1:5]
URL = "http://127.0.0.1:40841"
def telemetry():
    req = urllib.request.Request(URL + "/__rusty/product/runtime/diagnostics/read", data=b"{}",
                                 headers={"Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=5).read())["telemetry"]
runs = []
for run in range(2):
    proc = subprocess.Popen([host, "--product", product, "--loader", "coreclr"], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    deadline = time.time() + 60
    while time.time() < deadline:
        try:
            telemetry(); break
        except Exception:
            time.sleep(0.25)
    time.sleep(15)
    t = telemetry()
    ua = t.get("updateAttribution") or {}
    runs.append({
        "samples": int(ua.get("sampleCount", 0)),
        "callbackUsP50": int(ua.get("callbackDurationUsP50", 0)),
        "callbackUsP95": int(ua.get("callbackDurationUsP95", 0)),
        "latestPostCallbackUs": int((ua.get("latest") or {}).get("postCallbackDurationUs", 0)),
        "progress": t.get("progress"),
    })
    os.killpg(proc.pid, signal.SIGINT)
    proc.wait(timeout=30)
    time.sleep(1)
result = {"label": label, "runs": runs}
json.dump(result, open(out, "w"), indent=2)
print(json.dumps(result))
