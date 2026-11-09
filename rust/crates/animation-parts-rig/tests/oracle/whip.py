"""Writes `whip`: rig-c's gallery `look` without its two slider constraints
and with only its `turn` clip, so its `whip` physics constraint (the
ahoge's spring) plays under the head turn the slider drove, as an ordinary
clip.

    python3 -I whip.py <look skeleton.model.json> <out skeleton.model.json>
"""
import json
import sys

doc = json.load(open(sys.argv[1]))
doc["constraints"] = [c for c in doc["constraints"] if c["kind"] != "slider"]
for skin in doc["skins"]:
    skin["constraints"]["slider"] = []
doc["animations"] = [a for a in doc["animations"] if a["name"] == "turn"]
doc["editorOrder"]["animations"] = ["turn"]
for animation in doc["animations"]:
    animation["constraints"]["slider"] = []
json.dump(doc, open(sys.argv[2], "w"), indent=2)
