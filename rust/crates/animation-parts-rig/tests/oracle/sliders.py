"""Writes `sliders`: the build of `sliders.rig.json` and `sliders.motion.json`
with each helper clip `<a>_x` merged into the slider animation `<a>` and
removed. rigc's gates play every clip with its sliders applied, so a slider
animation cannot key an `rgb2`, an `alpha` or a sequence timeline in the
spec without failing them; merged after the build, the sliders apply them.

    python3 -I sliders.py <built skeleton.model.json> <out skeleton.model.json>
"""
import json
import sys

doc = json.load(open(sys.argv[1]))
by_name = {a["name"]: a for a in doc["animations"]}
for name in [n for n in by_name if n.endswith("_x")]:
    extra, into = by_name[name], by_name[name[:-2]]
    for key in ("bones", "slots", "attachments"):
        into[key] += extra[key]
    into["duration"] = max(into["duration"], extra["duration"])
    doc["animations"].remove(extra)
    doc["editorOrder"]["animations"].remove(name)
json.dump(doc, open(sys.argv[2], "w"), indent=2)
