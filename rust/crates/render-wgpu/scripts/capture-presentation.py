#!/usr/bin/env python3
"""Capture one fresh-attachment presentation baseline from a running product.

Usage: capture-presentation.py [origin] [output-dir]
  origin      default http://127.0.0.1:4395 (Doom `scripts/run-room-study.sh`)
  output-dir  default target/render-wgpu-capture under the repository root

Writes world-frame.json (the presentation-world frame), view.json (camera
composition) and resources/ (every texture and mesh resource the frame names).
Render it with `cargo run -p render-wgpu --example render_capture`.
"""

import json
import pathlib
import sys
import urllib.parse
import urllib.request

origin = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:4395"
out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[4] / "target" / "render-wgpu-capture"
headers = {"Origin": origin, "Accept": "text/event-stream"}

fragments, counts, event = {}, {}, None
request = urllib.request.Request(f"{origin}/__rusty/product/runtime/outputs/fresh", headers=headers)
with urllib.request.urlopen(request, timeout=30) as stream:
    for raw in stream:
        line = raw.decode().rstrip("\n")
        if line.startswith("event:"):
            event = line[6:].strip()
        elif line.startswith("data:") and event == "rusty-output-fragment":
            data = json.loads(line[5:])
            fragments.setdefault(data["transferId"], {})[data["fragmentIndex"]] = data["data"]
            counts[data["transferId"]] = data["fragmentCount"]
            if len(fragments[data["transferId"]]) == data["fragmentCount"]:
                transfer = data["transferId"]
                break

batch = json.loads("".join(fragments[transfer][i] for i in range(counts[transfer])))
outputs = {output["kind"]: output for output in batch["outputs"]}
generation = outputs["binding"]["runtime"]["generation"]
frame = outputs["frame"]["frame"]

out.mkdir(parents=True, exist_ok=True)
(out / "world-frame.json").write_text(json.dumps(frame))
(out / "view.json").write_text(json.dumps(outputs["view-composition"]["composition"]))

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
for name in sorted(names):
    query = urllib.parse.urlencode({"identity": name, "generation": generation}, safe="")
    request = urllib.request.Request(f"{origin}/__rusty/product/runtime/resource?{query}", headers={"Origin": origin})
    with urllib.request.urlopen(request, timeout=30) as response:
        (resources / name.replace("/", "__")).write_bytes(response.read())

print(f"{len(frame['ops'])} ops, {len(names)} resources -> {out}")
