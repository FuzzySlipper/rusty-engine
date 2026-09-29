# Usage: run-exercise.py <rusty-product-host> <staged Product dir> <changes|snapshot> <out.json>
# Runs the graphics exercise under a headless browser until it reports, records
# its facts line and error diagnostics, then attaches a fresh renderer (a new
# output stream) and checks the retained baseline it receives.
import json, os, signal, socket, subprocess, sys, time, urllib.request

host, product, mode, out = sys.argv[1:5]
PORT = 40851
URL = f"http://127.0.0.1:{PORT}"
errors = []


def diagnostics():
    request = urllib.request.Request(URL + "/__rusty/product/runtime/diagnostics/read", data=b"{}",
                                     headers={"Content-Type": "application/json"})
    read = json.loads(urllib.request.urlopen(request, timeout=5).read())
    for event in read.get("events", []):
        if event.get("severity") in ("error", "fatal") and event["message"] not in errors:
            errors.append(event["message"])


def fresh_baseline():
    """Reads the baseline event a newly attached renderer receives."""
    stream = socket.create_connection(("127.0.0.1", PORT), timeout=10)
    stream.sendall(f"GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\n"
                   "Accept: text/event-stream\r\nConnection: keep-alive\r\n\r\n".encode())
    data = b""
    while b"event: rusty-output-baseline" not in data or b"\n\n" not in data[data.find(b"event: rusty-output-baseline"):]:
        chunk = stream.recv(1 << 20)
        if not chunk:
            break
        data += chunk
    stream.close()
    text = data.decode("utf-8", "replace")
    open(os.environ.get("BASELINE_DUMP", os.devnull), "w").write(text)
    # The baseline is every output batch before the baseline marker event.
    batches = []
    for block in text[:text.find("event: rusty-output-baseline")].split("\n\n"):
        payload = "".join(line[len("data:"):].strip() for line in block.splitlines() if line.startswith("data:"))
        if payload:
            batches.append(json.loads(payload))
    return batches


def walk(value, visit):
    if isinstance(value, dict):
        visit(value)
        for item in value.values():
            walk(item, visit)
    elif isinstance(value, list):
        for item in value:
            walk(item, visit)


env = dict(os.environ, EXERCISE_PUBLISH=mode)
process = subprocess.Popen([host, "--product", product, "--loader", "coreclr", "--headless"],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                           text=True, start_new_session=True, env=env)
os.set_blocking(process.stdout.fileno(), False)
lines, facts, verdict = [], None, None
deadline = time.time() + 150
while time.time() < deadline and verdict is None:
    time.sleep(0.5)
    try:
        chunk = process.stdout.read() or ""
    except BlockingIOError:
        chunk = ""
    for line in chunk.splitlines():
        lines.append(line)
        if line.startswith("GRAPHICS_FACTS "):
            facts = json.loads(line[len("GRAPHICS_FACTS "):])
        if line.startswith("GRAPHICS_EXERCISE_"):
            verdict = line
    try:
        diagnostics()
    except Exception:
        pass

baseline = {}
try:
    frame = fresh_baseline()
    counts = {}
    joints, hidden = [], 0
    mesh_definitions = set()

    def visit(node):
        global hidden
        kind = node.get("op")
        if not isinstance(kind, str):
            return
        counts[kind] = counts.get(kind, 0) + 1
        if kind == "setParentJoint":
            joints.append(node.get("joint"))
        if kind in ("create", "createStaticMeshInstance", "createAnimatedMeshInstance"):
            body = node.get("node") or node.get("instance") or {}
            if body.get("visible") is False:
                hidden += 1
        if kind == "defineStaticMesh":
            mesh_definitions.add((node.get("asset") or {}).get("asset"))
    walk(frame, visit)
    baseline = {"operationCounts": counts, "joints": joints, "hiddenObjects": hidden,
                "staticMeshDefinitions": sorted(filter(None, mesh_definitions))}
except Exception as error:
    baseline = {"error": repr(error)}
try:
    diagnostics()
except Exception:
    pass
os.killpg(process.pid, signal.SIGINT)
process.wait(timeout=30)
result = {
    "mode": mode,
    "verdict": verdict,
    "facts": facts,
    "freshRendererBaseline": baseline,
    "errors": [error[:600] for error in errors],
    "log": [line for line in lines if "gcm" not in line][:40],
}
json.dump(result, open(out, "w"), indent=2)
print(json.dumps({key: result[key] for key in ("verdict", "facts", "freshRendererBaseline", "errors")}, indent=2))
