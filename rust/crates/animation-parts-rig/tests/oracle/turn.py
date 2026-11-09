"""Writes `turned`: a squash model whose page regions are trimmed and turned
(90, 180 and 270 degrees), for the page UVs only.

    python3 -I turn.py <squash skeleton.model.json> <out skeleton.model.json>
"""
import json
import sys

doc = json.load(open(sys.argv[1]))
changes = {
    "ball": lambda r: {"width": r["width"] - 14, "height": r["height"] - 20, "offsetX": 6, "offsetY": 10, "degrees": 90},
    "head": lambda r: {"width": r["width"] - 8, "height": r["height"] - 4, "offsetX": 3, "offsetY": 1, "degrees": 180},
    "shadow": lambda r: {"degrees": 90},
    "torso": lambda r: {"width": r["width"] - 6, "height": r["height"] - 2, "offsetX": 2, "offsetY": 2, "degrees": 270},
    "arm_f": lambda r: {"width": r["width"] - 4, "offsetX": 1},
}
for region in doc["pages"][0]["regions"]:
    if region["name"] in changes:
        region.update(changes[region["name"]](region))
json.dump(doc, open(sys.argv[2], "w"), indent=2)
