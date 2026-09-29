"""Drive Doom's player through waypoints over live-debug, then use a door."""
import json, math, re, subprocess, sys, time, urllib.request

origin, sse_path, live_debug = sys.argv[1], sys.argv[2], sys.argv[3]
waypoints = [(-1.0, 1.0), (0.6, -17.0), (0.6, -30.0), (4.6, -30.6)]

def debug(command):
    out = subprocess.run([live_debug, "--origin", origin, "--command", command], capture_output=True, text=True, timeout=30)
    return out.stdout.strip()

def post_input(fact):
    stream = open(sse_path).read().replace('\\"', '"')
    runtime = json.loads(re.findall(r'\{"kind":"binding","runtime":(\{[^}]*\})', stream)[-1])
    sequence = re.findall(r'"nextInputSequence":"(\d+)"', stream)[-1]
    body = json.dumps({"batch": [{"runtime": runtime, "sequence": sequence, "context": "gameplay.default", "fact": fact}]}).encode()
    request = urllib.request.Request(origin + "/__rusty/product/runtime/input", body, {"Content-Type": "application/json", "Origin": origin})
    urllib.request.urlopen(request, timeout=10).read()
    time.sleep(0.15)

def observe():
    facts = json.loads(debug("playtest.observe"))
    p = facts["player"]
    return p["position"]["x"], p["position"]["z"], p["yawDegrees"]

for tx, tz in waypoints:
    for _ in range(80):
        x, z, yaw = observe()
        dx, dz = tx - x, tz - z
        if math.hypot(dx, dz) < 0.6:
            break
        # yaw 0 faces -Z, positive turns right toward +X.
        want = math.degrees(math.atan2(dx, -dz))
        turn = (want - yaw + 540) % 360 - 180
        if abs(turn) > 2:
            debug(f"playtest.look {turn:.2f} 0")
        post_input({"kind": "key", "code": "key-w", "edge": "pressed"})
        time.sleep(min(0.6, 0.12 * math.hypot(dx, dz)))
        nx, nz, _ = observe()
        if math.hypot(nx - x, nz - z) < 0.05:
            # Blocked by a ledge: jump while still moving forward.
            post_input({"kind": "key", "code": "space", "edge": "pressed"})
            time.sleep(0.4)
            post_input({"kind": "key", "code": "space", "edge": "released"})
        post_input({"kind": "key", "code": "key-w", "edge": "released"})
    print("reached", tx, tz, "at", observe(), flush=True)

inspect = json.loads(debug("interaction.inspect"))
door = next(c for c in inspect["candidates"] if c["id"] == 20000)
print("door", {k: door.get(k) for k in ("distance", "reachDistance", "revision")}, flush=True)
print(debug(f"interaction.use 20000 {door['revision']}"), flush=True)
time.sleep(2)
