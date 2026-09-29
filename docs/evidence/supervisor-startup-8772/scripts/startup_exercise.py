"""#8772 exercise: stop or replace a runtime that has not reported ready.

python3 startup_exercise.py HOST PRODUCT PORT WORK OUT_JSON

The supervisor is launched --supervised, as `rusty dev` does. Its runtime child
is stopped with SIGSTOP as soon as it appears, so it never reports ready. Each
case then asks the supervisor to act and records how long it took:
- groupSigint: SIGINT to the supervisor's process group (terminal Ctrl+C);
- stdinEof: close the supervisor's stdin (the `rusty dev` clean stop);
- restage: a replace-runtime frame; the new runtime must serve.
"""
import json, os, signal, socket, struct, subprocess, sys, time
from pathlib import Path

host, product, port, work, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5])
work.mkdir(parents=True, exist_ok=True)
ADDR = ("127.0.0.1", port)
WAIT = 30


def status(path="/"):
    try:
        s = socket.create_connection(ADDR, timeout=2)
        s.sendall(f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n".encode())
        head = s.recv(64)
        s.close()
        return int(head.split(b" ")[1]) if head else 0
    except OSError:
        return None


def children(pid):
    try:
        return [int(c) for c in Path(f"/proc/{pid}/task/{pid}/children").read_text().split()]
    except FileNotFoundError:
        return []


def alive(pid):
    try:
        return Path(f"/proc/{pid}/stat").read_text().split()[2] != "Z"
    except FileNotFoundError:
        return False


def frame(command):
    body = json.dumps(command).encode()
    return struct.pack("<I", len(body)) + body


def held_runtime(name):
    log = (work / f"{name}.log").open("w")
    proc = subprocess.Popen([host, "--product", product, "--loader", "coreclr", "--supervised",
                             "--persistence-root", str(work / f"p-{name}"),
                             "--content-store-root", str(work / f"cs-{name}")],
                            stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT,
                            start_new_session=True)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not children(proc.pid):
        time.sleep(0.005)
    runtime = children(proc.pid)[0]
    os.kill(runtime, signal.SIGSTOP)
    time.sleep(1.0)
    return proc, runtime


def wait_exit(proc):
    try:
        return proc.wait(timeout=WAIT)
    except subprocess.TimeoutExpired:
        return None


def finish(proc, runtime, name, started, code):
    seconds = round(time.monotonic() - started, 3)
    reaped = not alive(runtime)
    if code is None:
        os.killpg(proc.pid, signal.SIGKILL)
        os.kill(runtime, signal.SIGKILL)
        proc.wait()
    lines = [l for l in (work / f"{name}.log").read_text().splitlines() if "RUSTY_HOST" in l]
    return {"supervisorExited": code is not None, "exitCode": code, "seconds": seconds,
            "heldRuntimeReaped": reaped, "log": lines[-3:]}


result = {}

proc, runtime = held_runtime("group-sigint")
result["pageWhileHeld"] = status()
started = time.monotonic()
os.killpg(proc.pid, signal.SIGINT)
result["groupSigint"] = finish(proc, runtime, "group-sigint", started, wait_exit(proc))

proc, runtime = held_runtime("stdin-eof")
started = time.monotonic()
proc.stdin.close()
result["stdinEof"] = finish(proc, runtime, "stdin-eof", started, wait_exit(proc))

proc, runtime = held_runtime("restage")
started = time.monotonic()
proc.stdin.write(frame({"kind": "replace-runtime", "productDirectory": product}))
proc.stdin.flush()
served = None
while time.monotonic() - started < WAIT:
    if status() == 200:
        served = round(time.monotonic() - started, 3)
        break
    time.sleep(0.05)
replacement = [c for c in children(proc.pid) if c != runtime]
result["restage"] = {"servedAfterSeconds": served, "heldRuntimeReaped": not alive(runtime),
                     "replacementRuntime": bool(replacement)}
started = time.monotonic()
proc.stdin.close()
code = wait_exit(proc)
result["restage"]["thenStdinEof"] = finish(proc, runtime, "restage", started, code)

out.write_text(json.dumps(result, indent=2))
print(json.dumps(result, indent=2))
