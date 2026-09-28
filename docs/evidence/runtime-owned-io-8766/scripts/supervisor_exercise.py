"""#8766 supervisor exercise against a staged Product (no browser).

python3 supervisor_exercise.py HOST PRODUCT PORT WORK OUT_JSON

Supervised (rusty dev style): replacement, automatic restart after a runtime
crash, failure pause answered with 503, recovery by replacement, stdin-close
stop. Standalone: a runtime crash stops the host with a named nonzero exit.
"""
import json, os, signal, socket, struct, subprocess, sys, threading, time
from pathlib import Path

host, product, port, work, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5])
work.mkdir(parents=True, exist_ok=True)
ADDR = ("127.0.0.1", port)


def request(path, accept="application/json", timeout=2.0):
    s = socket.create_connection(ADDR, timeout=timeout)
    s.sendall(f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: {accept}\r\nConnection: close\r\n\r\n".encode())
    data = b""
    try:
        while True:
            chunk = s.recv(65536)
            if not chunk:
                break
            data += chunk
    except socket.timeout:
        pass
    s.close()
    head, _, body = data.partition(b"\r\n\r\n")
    return int(head.split(b" ")[1]) if head else 0, body


def fresh_binding(timeout=5.0):
    """Open /outputs/fresh and return the first binding's runtime identity."""
    s = socket.create_connection(ADDR, timeout=timeout)
    s.sendall(f"GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n".encode())
    buf = b""
    deadline = time.monotonic() + timeout
    try:
        while time.monotonic() < deadline:
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
            for block in buf.split(b"\n\n"):
                for line in block.split(b"\n"):
                    if line.startswith(b"data: ") and b'"binding"' in line:
                        for output in json.loads(line[6:]).get("outputs", []):
                            if output.get("kind") == "binding":
                                return output["runtime"]
    finally:
        s.close()
    return None


def children(pid):
    try:
        return [int(c) for c in Path(f"/proc/{pid}/task/{pid}/children").read_text().split()]
    except FileNotFoundError:
        return []


def frame(command):
    body = json.dumps(command).encode()
    return struct.pack("<I", len(body)) + body


def watch_statuses(stop, log):
    while not stop.is_set():
        started = time.monotonic()
        try:
            status, _ = request("/__rusty/product/runtime/debug/catalog", timeout=1.0)
        except OSError:
            status = -1
        log.append((round(started, 3), status))
        time.sleep(0.02)


def wait_for(predicate, timeout, step=0.05):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            value = predicate()
        except OSError:
            value = None
        if value:
            return value
        time.sleep(step)
    return None


def spawn(extra, name):
    log = (work / f"{name}.log").open("w")
    process = subprocess.Popen([host, "--product", product, "--loader", "coreclr", *extra,
                                "--persistence-root", str(work / f"p-{name}"),
                                "--content-store-root", str(work / f"cs-{name}")],
                               stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT,
                               start_new_session=True)
    return process


def gap(statuses, since):
    after = [(t, s) for t, s in statuses if t >= since]
    unavailable = [t for t, s in after if s == 503]
    return {"requests": len(after), "answered503": len(unavailable),
            "other": sorted({s for _, s in after if s not in (200, 503)})}


result = {}
# ---- supervised ----
proc = spawn(["--supervised", "--runtime-instance-id", "100"], "supervised")
statuses, stop = [], threading.Event()
first = wait_for(lambda: fresh_binding() if request("/")[0] == 200 else None, 60)
result["initial"] = {"binding": first, "runtime": children(proc.pid)}
threading.Thread(target=watch_statuses, args=(stop, statuses), daemon=True).start()

# Replacement, as rusty dev writes it after restaging.
old = children(proc.pid)
t0 = time.monotonic()
proc.stdin.write(frame({"kind": "replace-runtime", "productDirectory": product})); proc.stdin.flush()
replaced = wait_for(lambda: (b := fresh_binding(1.0)) and b != first and b, 60)
result["replace"] = {"binding": replaced, "seconds": round(time.monotonic() - t0, 3),
                     "oldRuntimeGone": not Path(f"/proc/{old[0]}").exists(), "runtime": children(proc.pid),
                     "window": gap(statuses, t0)}

# Crash 1: one automatic restart.
victim = children(proc.pid)[0]
t0 = time.monotonic()
os.kill(victim, signal.SIGKILL)
restarted = wait_for(lambda: (b := fresh_binding(1.0)) and b != replaced and b, 60)
result["crashRestart"] = {"binding": restarted, "seconds": round(time.monotonic() - t0, 3),
                          "window": gap(statuses, t0)}

# Crash 2: paused, answered with 503 until restage.
victim = children(proc.pid)[0]
t0 = time.monotonic()
os.kill(victim, signal.SIGKILL)
time.sleep(2.0)
api_status, api_body = request("/__rusty/product/runtime/debug/catalog")
page_status, page_body = request("/", accept="text/html")
result["paused"] = {"runtime": children(proc.pid), "api": [api_status, api_body.decode()[:300]],
                    "page": [page_status, page_body.decode()[:200]]}

# Recovery by the next restage.
t0 = time.monotonic()
proc.stdin.write(frame({"kind": "replace-runtime", "productDirectory": product})); proc.stdin.flush()
recovered = wait_for(lambda: fresh_binding(1.0), 60)
result["recover"] = {"binding": recovered, "seconds": round(time.monotonic() - t0, 3)}

stop.set()
t0 = time.monotonic()
last = children(proc.pid)
proc.stdin.close()
code = proc.wait(timeout=30)
result["stdinClose"] = {"exitCode": code, "seconds": round(time.monotonic() - t0, 3),
                        "runtimeReaped": all(not Path(f"/proc/{p}").exists() for p in last)}

# ---- standalone ----
proc = spawn([], "standalone")
wait_for(lambda: request("/")[0] == 200, 60)
victim = children(proc.pid)[0]
os.kill(victim, signal.SIGKILL)
code = proc.wait(timeout=30)
result["standaloneCrash"] = {"exitCode": code,
                             "log": [l for l in (work / "standalone.log").read_text().splitlines() if "RUSTY_HOST" in l or "Error" in l][-3:]}
out.write_text(json.dumps(result, indent=2))
print(json.dumps(result, indent=2))
