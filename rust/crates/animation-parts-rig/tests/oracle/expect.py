"""Writes the fixtures `tests/rigc_core.rs` compares against: each rig's model
document, and the poses rig-c's own core gives for its cases.

    python3 -I expect.py <bun> <core-entry.ts> <oracle.ts> <out-dir> <dt> <cap> <name=skeleton.model.json>...

`dt` is the step in seconds and `cap` the most steps one looping case takes.
Each rig gets, under no skin and under each named skin, one looping case per
clip (rig-c's `openTrack`) and one crossfade script over its first two clips
without draw-order, sequence or constraint keys (`openTracks`). A rig with a
bone named `turn` also gets its first clip with `turn`'s x driven to 24 and
then -24 (rig-c's `adjust`). A rig with physics constraints also gets its
first clip walked while the rig moves, `move` units a step to the right for
a third of the steps, then held, under a gust: a sideways wind from the
second third on, turned off for the last (rig-c's `physicsTranslate` and
`setEnvironment`). A rig named `turned` gets one two-pose case instead: it
is a page-UV variant. Bones keep full doubles;
vertices are written to 8 significant digits, UVs and colours to 9.
"""
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

bun, entry, oracle, out_dir, dt, cap = sys.argv[1:7]
dt = float(dt)
cap = int(cap)
rigs = [a.split("=", 1) for a in sys.argv[7:]]
# The physics case: rig units the rig moves right per step, and the gust's
# wind and gravity (x right, y down).
MOVE = 3.0
GUST = {"wind": [4.0, 0.0], "gravity": [0.0, 1.5]}
CALM = {"wind": [0.0, 0.0], "gravity": [0.0, 1.0]}
out = Path(out_dir)
out.mkdir(parents=True, exist_ok=True)


def g(v, digits=9):
    return None if v is None else float(f"{v:.{digits}g}")


def run(plan):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(plan, f)
        path = f.name
    result = subprocess.run([bun, oracle, entry, path], capture_output=True, text=True, check=True)
    Path(path).unlink()
    return json.loads(result.stdout)["poses"]


def keep(regions, d):
    key = f"{d['slot']}|{d['attachment']}|{d['region']}"
    regions.setdefault(key, {"page": d["page"], "uvs": [g(v) for v in d["uvs"]], "triangles": d["triangles"]})
    return key


def compact(poses, regions):
    return [
        {
            "time": p["animationTime"],
            "bones": [x for b in p["bones"] for x in b],
            "drawn": [
                [
                    keep(regions, d),
                    [g(v, 8) for v in d["vertices"]],
                    [g(v) for v in d["color"]],
                    None if d["dark"] is None else [g(v) for v in d["dark"]],
                    d["blend"],
                ]
                for d in p["drawn"]
                if d["active"]
            ],
            "events": p["events"],
        }
        for p in poses
    ]


def track(model, skin, anim, steps, regions, name, drive=None, before=None):
    plan = {"model": model, "skin": skin, "mode": "track", "animation": anim, "loop": True, "steps": steps}
    case = {"name": name, "skin": skin, "mode": "track", "animation": anim, "loop": True, "steps": steps}
    if drive:
        plan["drive"] = case["drive"] = drive
    if before:
        plan["before"] = case["before"] = before
    case["poses"] = compact(run(plan), regions)
    return case


for name, model in rigs:
    regions = {}
    doc = json.loads(Path(model).read_text())
    (out / f"{name}.model.json").write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    anims = [a["name"] for a in doc["animations"]]
    if name == "turned":
        cases = [track(model, None, anims[0], [0.3], regions, f"{anims[0]} on turned regions")]
        skins = []
    else:
        cases = []
        skins = [None] + [s["name"] for s in doc["skins"] if s["name"] != "default"]
    mixable = [
        a["name"] for a in doc["animations"]
        if not a["drawOrder"] and not any("sequence" in at for sk in a["attachments"] for sl in sk["slots"] for at in sl["attachments"])
        and not any(a.get("constraints", {}).get(k) for k in ("ik", "transform", "physics"))
    ]
    turn = any(b["name"] == "turn" for b in doc["bones"])
    physics = any(c["kind"] == "physics" for c in doc["constraints"])
    for skin in skins:
        for i, anim in enumerate(anims):
            duration = max([0.0] + [k["time"] for t in doc["animations"][i]["bones"] for tl in t["timelines"] for k in tl["keys"]])
            steps = [dt] * max(4, min(cap, math.ceil(1.4 * max(duration, 0.5) / dt)))
            cases.append(track(model, skin, anim, steps, regions, f"{anim} looping"))
            if turn and i == 0:
                for x in (24, -24):
                    cases.append(track(model, skin, anim, steps, regions, f"{anim} with turn at {x}", [["turn", "x", x]]))
            if physics and i == 0:
                third = max(1, len(steps) // 3)
                before = {str(k + 1): {"move": [MOVE, 0.0]} for k in range(third)}
                before.setdefault(str(third + 1), {}).update(GUST)
                before.setdefault(str(2 * third + 1), {}).update(CALM)
                cases.append(track(model, skin, anim, steps, regions, f"{anim} moving in a gust", before=before))
        if mixable:
            a, b = mixable[0], mixable[min(1, len(mixable) - 1)]

            def hold(seconds):
                return [{"step": dt}] * max(2, round(seconds / dt))

            ops = [{"play": a, "loop": True, "mix": 0}, {"step": 0}] + hold(0.3)
            ops += [{"play": b, "loop": True, "mix": 0.3}] + hold(0.45)
            ops += [{"play": a, "loop": False, "mix": 0.2}] + hold(0.1)
            ops += [{"play": b, "loop": True, "mix": 0.25}] + hold(0.35)
            ops += [{"stop": 0.3}] + hold(0.4)
            plan = {"model": model, "skin": skin, "mode": "tracks", "ops": ops}
            cases.append({"name": f"crossfade {a} {b}", "skin": skin, "mode": "tracks", "ops": ops,
                          "poses": compact(run(plan), regions)})
    (out / f"{name}.expected.json").write_text(json.dumps({"regions": regions, "cases": cases}, separators=(",", ":")) + "\n")
    print(name, len(cases), "cases")
