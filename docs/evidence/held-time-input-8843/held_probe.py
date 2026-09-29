"""Held-time movement without a browser: physical W through the runtime input
route, then `engine.time.advance`, then release. Compare with realtime.

usage: held_probe.py <origin> <outputs-sse-file> <live-debug>
"""
import json, re, subprocess, sys, time, urllib.request

ORIGIN, STREAM, LIVE = sys.argv[1:4]


def debug(command):
    out = subprocess.run([LIVE, "--origin", ORIGIN, "--command", command], capture_output=True, text=True).stdout
    return out.strip()


def position():
    fields = dict(p.split("=", 1) for p in debug("loading-bay.readout").split(";") if "=" in p)
    return tuple(round(float(v), 3) for v in fields["position"].split(",")), fields.get("step")


def key(code, edge):
    stream = open(STREAM).read().replace('\\"', '"')
    runtime = json.loads(re.findall(r'\{"kind":"binding","runtime":(\{[^}]*\})', stream)[-1])
    sequence = re.findall(r'"nextInputSequence":"(\d+)"', stream)[-1]
    event = {"runtime": runtime, "sequence": sequence, "context": "gameplay.default",
             "fact": {"kind": "key", "code": code, "edge": edge}}
    request = urllib.request.Request(f"{ORIGIN}/__rusty/product/runtime/input",
                                     data=json.dumps({"batch": [event]}).encode(),
                                     headers={"Content-Type": "application/json", "Origin": ORIGIN})
    reply = urllib.request.urlopen(request).read().decode()
    print(f"  {code} {edge}: {reply[:160]}")
    time.sleep(0.1)


def trial(mode):
    print(f"== {mode}")
    print("  time:", debug(f"engine.time.mode {mode}"))
    before = position()
    key("key-w", "pressed")
    if mode == "realtime":
        time.sleep(0.5)
    else:
        print("  advance:", debug("engine.time.advance 500"))
    key("key-w", "released")
    after = position()
    print(f"  {before} -> {after}")


trial("realtime")
trial("action-driven")
trial("manual")
print("  time:", debug("engine.time.mode realtime"))
