#!/usr/bin/env python3
"""Capture one fresh-attachment presentation baseline from a running product.

Usage: capture-presentation.py [origin] [output-dir]
  origin      default http://127.0.0.1:4395 (Doom `scripts/run-room-study.sh`)
  output-dir  default target/render-wgpu-capture under the repository root

Writes world-frame.json (the presentation-world frame), view.json (camera
composition), presentation.json (the baseline's presentation frames: billboards,
particles, ghost plates) and resources/ (every texture, mesh and font resource
the frames name).
Render it with `cargo run -p render-wgpu --example render_capture`.
"""

import json
import pathlib
import re
import sys
import urllib.parse
import urllib.request

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:4395"
out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[4] / "target" / "render-wgpu-capture"
headers = {"Origin": origin, "Accept": "text/event-stream"}

# The baseline batch arrives whole as a plain message (newer hosts) or as
# fragments of one transfer (`rusty-output-fragment`).
fragments, counts, event, batch = {}, {}, None, None
request = urllib.request.Request(f"{origin}/__rusty/product/runtime/outputs/fresh", headers=headers)
with urllib.request.urlopen(request, timeout=30) as stream:
    for raw in stream:
        line = raw.decode().rstrip("\n")
        if line.startswith("event:"):
            event = line[6:].strip()
        elif line == "":
            event = None
        elif line.startswith("data:") and event in (None, "message"):
            data = json.loads(line[5:])
            if data.get("kind") == "runtime-output-batch" and any(
                output["kind"] == "frame" for output in data["outputs"]
            ):
                batch = data
                break
        elif line.startswith("data:") and event == "rusty-output-fragment":
            data = json.loads(line[5:])
            fragments.setdefault(data["transferId"], {})[data["fragmentIndex"]] = data["data"]
            counts[data["transferId"]] = data["fragmentCount"]
            if len(fragments[data["transferId"]]) == data["fragmentCount"]:
                transfer = data["transferId"]
                batch = json.loads("".join(fragments[transfer][i] for i in range(counts[transfer])))
                break

outputs = {output["kind"]: output for output in batch["outputs"]}
generation = outputs["binding"]["runtime"]["generation"]
frame = outputs["frame"]["frame"]

out.mkdir(parents=True, exist_ok=True)
(out / "world-frame.json").write_text(json.dumps(frame))
(out / "view.json").write_text(json.dumps(outputs["view-composition"]["composition"]))
presentation = [output["frame"] for output in batch["outputs"] if output["kind"] == "presentation"]
(out / "presentation.json").write_text(json.dumps(presentation))

resources = out / "resources"
resources.mkdir(exist_ok=True)
names = set()
for op in frame["ops"]:
    if op["op"] == "defineTexture":
        source = op["texture"].get("payload", {}).get("source", {})
    elif op["op"] == "defineStaticMesh":
        source = op["asset"]["payload"]["source"]
    elif op["op"] == "replaceMeshPayload":
        source = op["payload"]["source"]
    else:
        continue
    if source.get("kind") == "resource":
        names.add(source["resource"])
# Billboard icons are texture resources named by content hash; asset fonts by
# their asset identity.
for presentation_frame in presentation:
    for op in presentation_frame["ops"]:
        if op["domain"] != "billboard":
            continue
        text = json.dumps(op["op"])
        for texture in re.findall(r'"contentHash": "sha256:([0-9a-f]{64})"', text):
            names.add(f"texture-resource/{texture}")
        names.update(re.findall(r'"asset": "(font/[^"]+)"', text))
# Animated mesh GLBs and clip packs are named only in the binding's list.
names.update(outputs["binding"].get("rendererResources", []))
for name in sorted(names):
    query = urllib.parse.urlencode({"identity": name, "generation": generation}, safe="")
    request = urllib.request.Request(f"{origin}/__rusty/product/runtime/resource?{query}", headers={"Origin": origin})
    with urllib.request.urlopen(request, timeout=30) as response:
        (resources / name.replace("/", "__")).write_bytes(response.read())

print(f"{len(frame['ops'])} ops, {len(names)} resources -> {out}")
