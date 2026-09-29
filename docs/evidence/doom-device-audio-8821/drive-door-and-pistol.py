#!/usr/bin/env python3
"""Drives Doom's room study from spawn to the north-wing door and opens it
with the ordinary use key. On the way it shoots the trooper that guards the
corridor. The trooper sees the player from the north walkway and kills it at
100 health before the corridor. The run therefore sets armor to 100 and tops
health back up to 100 (the room study's authored maximum) before each leg and
shot with `loading-bay.set-track`, a debug command. This is a sound
recording, not a combat test. It fires one pistol shot at spawn,
before any enemy is awake, and the corridor shots add more. Run it through
`device-proof.sh ... script:<this file>`, which passes the origin, the output
stream capture and the live-debug client.

Movement is ordinary physical keyboard input: `playtest.look` turns through
the product's look rules, then W is held for the leg's length. The waypoints
were read from `spatial.map-at` (the static route query returns NoPath here).
Each printed event carries seconds since this script started, so it can be
matched to the recording.
"""

import json
import math
import re
import subprocess
import sys
import time
import urllib.request

ORIGIN, STREAM, LIVE_DEBUG = sys.argv[1:4]
START = time.monotonic()
MOVEMENT_SPEED = 6.0  # LoadingBayTuning.MovementSpeed, units per second
# Feet X/Z on the level walkway, around the spawn room's recessed centre:
# north off the spawn step, west, up the west side, east along the north
# walkway to the corridor, north to the dogleg, then east through the
# vestibule to the door.
WAYPOINTS = [(-7.0, -2.0), (-13.0, -2.0), (-13.0, -16.5), (-0.5, -16.5), "clear",
             (-0.5, -29.5), (1.5, -30.8), (4.5, -31.8), (6.6, -32.4)]
MAXIMUM_SHOTS = 12
DOOR = 20000


def log(event, **fields):
    print(json.dumps({"t": round(time.monotonic() - START, 2), "event": event, **fields}), flush=True)


def debug(command):
    out = subprocess.run([LIVE_DEBUG, "--origin", ORIGIN, "--command", command],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out[out.index("{"):]) if "{" in out else out


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
    if '"accepted":true' not in reply.replace(" ", ""):
        log("input-refused", code=code, edge=edge, reply=reply[:400])
    time.sleep(0.1)  # let the input result advance the sequence in the stream


def tap(code, hold=0.1):
    key(code, "pressed")
    time.sleep(hold)
    key(code, "released")


def pose():
    player = debug("playtest.observe")["player"]
    position = player["position"]
    return (position["x"], position["y"], position["z"]), player["yawDegrees"]


def face(yaw_target):
    _, yaw = pose()
    delta = (yaw_target - yaw + 180) % 360 - 180
    if abs(delta) > 0.5:
        debug(f"playtest.look {delta:.2f} 0")


def heal():
    debug("loading-bay.set-track health 100")


def walk_to(x, z):
    for _ in range(8):
        heal()
        (px, _, pz), _ = pose()
        dx, dz = x - px, z - pz
        distance = math.hypot(dx, dz)
        if distance < 0.6:
            return
        # Yaw 0 faces -Z and positive yaw turns right (+X).
        face(math.degrees(math.atan2(dx, -dz)))
        key("key-w", "pressed")
        time.sleep(max(0.12, distance / MOVEMENT_SPEED))
        key("key-w", "released")
        time.sleep(0.15)
    (px, _, pz), _ = pose()
    if math.hypot(x - px, z - pz) > 1.5:
        observed = debug("playtest.observe")
        player = observed["player"]
        log("stalled", target=[x, z], position=[px, pz], readiness=observed.get("readiness"),
            health=player["health"], dead=player["dead"], lastDamage=player["lastDamage"],
            time=debug("engine.time"))
        raise SystemExit(1)


def clear_corridor():
    """Shoots the nearest living enemy until it is down."""
    for _ in range(MAXIMUM_SHOTS):
        observed = debug("playtest.observe")
        (px, _, pz), _ = pose()
        alive = [enemy for enemy in observed["enemies"] if enemy.get("health", 0) > 0]
        if not alive:
            return
        target = min(alive, key=lambda enemy: math.hypot(enemy["position"]["x"] - px, enemy["position"]["z"] - pz))
        if math.hypot(target["position"]["x"] - px, target["position"]["z"] - pz) > 12:
            return
        face(math.degrees(math.atan2(target["position"]["x"] - px, -(target["position"]["z"] - pz))))
        for _ in range(30):
            player = debug("playtest.observe")["player"]
            if player["weaponReady"]:
                break
            time.sleep(0.1)
        heal()
        tap("control-left")
        log("fire-pressed", target=target["id"], targetHealth=target["health"], ammo=player["ammo"],
            aimHit=player.get("aimHit"), aimAssist=player.get("aimAssist"))
        time.sleep(0.4)
    log("enemy-not-cleared")


def door_state():
    for target in debug("navigation.targets")["targets"]:
        if target.get("interactionEntity") == DOOR:
            return target["state"]
    return None


log("start", position=pose()[0], door=door_state())
debug("loading-bay.set-track armor 100")
player = debug("playtest.observe")["player"]
log("tracks", health=player["health"])
tap("control-left")
log("fire-pressed", target=None, ammo=player["ammo"])
time.sleep(1.5)
for step in WAYPOINTS:
    if step == "clear":
        clear_corridor()
        continue
    x, z = step
    walk_to(x, z)
    log("waypoint", target=[x, z], position=pose()[0])
def door_candidate():
    inspected = debug("interaction.inspect")
    door = next(c for c in inspected["candidates"] if c["id"] == DOOR)
    return inspected.get("selected"), door


for attempt in range(3):
    selected, door = door_candidate()
    log("door-candidate", selected=selected, distance=door["distance"], yawDelta=door["yawDeltaDegrees"],
        pitchDelta=door["pitchDeltaDegrees"], visibility=door["visibility"], reason=door["focusReason"])
    if selected is None:
        # Turn by the product's own aim guidance, then step toward the door.
        debug(f"playtest.look {door['yawDeltaDegrees']:.2f} {door['pitchDeltaDegrees']:.2f}")
        if door["distance"] > 2.0:
            tap("key-w", hold=0.25)
        selected, door = door_candidate()
    tap("key-e")
    log("use-pressed", selected=selected, reason=door["focusReason"])
    time.sleep(2.0)
    state = door_state()
    log("door", state=state, position=pose()[0])
    if state != "closed":
        break
player = debug("playtest.observe")["player"]
log("end", health=player["health"], ammo=player["ammo"], dead=player["dead"])
