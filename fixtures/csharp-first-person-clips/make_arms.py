#!/usr/bin/env python3
"""Splits Drillimpact's PSX first-person arms (CC0) into the fixture's arms and their clip pack.

`arms.glb` is the arms model with its resting and guard clips; `arms.knife.glb` is a clip pack that carries only the
arms' skeleton (its nodes and skin) and the knife and jab clips. The source is asset-pipeline's
`tools/fps-arms/packs/psx-drillimpact/arms_rig.glb`. A development tool: both files are committed, and a product never
runs this.

    python3 make_arms.py /path/to/arms_rig.glb content/arms.glb content/arms.knife.glb
"""

import json
import struct
import sys

PACKED = ["knife_draw", "knife_hit_01", "knife_hit_02", "jab.R"]


def read_glb(path):
    data = open(path, "rb").read()
    json_length = struct.unpack_from("<I", data, 12)[0]
    document = json.loads(data[20 : 20 + json_length])
    bin_offset = 20 + json_length
    bin_length = struct.unpack_from("<I", data, bin_offset)[0]
    return document, data[bin_offset + 8 : bin_offset + 8 + bin_length]


def write_glb(path, document, blob):
    blob = bytes(blob) + b"\0" * (-len(blob) % 4)
    document["buffers"] = [{"byteLength": len(blob)}]
    text = json.dumps(document, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    total = 12 + 8 + len(text) + 8 + len(blob)
    with open(path, "wb") as out:
        out.write(b"glTF" + struct.pack("<II", 2, total))
        out.write(struct.pack("<I", len(text)) + b"JSON" + text)
        out.write(struct.pack("<I", len(blob)) + b"BIN\0" + blob)


def main(source, arms_path, pack_path):
    document, blob = read_glb(source)
    names = [clip["name"] for clip in document["animations"]]
    missing = [name for name in PACKED if name not in names]
    if missing:
        raise SystemExit(f"the source has no clips {missing}")

    # The model: everything but the packed clips (their keys stay in the buffer unread).
    arms = dict(document)
    arms["animations"] = [clip for clip in document["animations"] if clip["name"] not in PACKED]
    write_glb(arms_path, arms, blob)

    # The pack: the skeleton and the packed clips, each accessor copied into a buffer of its own.
    out_bin = bytearray()
    views, accessors = [], []

    def copy_accessor(index):
        accessor = dict(document["accessors"][index])
        view = document["bufferViews"][accessor.pop("bufferView")]
        start = view.get("byteOffset", 0) + accessor.pop("byteOffset", 0)
        width = {"SCALAR": 1, "VEC3": 3, "VEC4": 4, "MAT4": 16}[accessor["type"]]
        size = {5126: 4, 5123: 2, 5121: 1}[accessor["componentType"]]
        length = accessor["count"] * width * size
        stride = view.get("byteStride")
        if stride and stride != width * size:
            raise SystemExit("interleaved accessors are not copied")
        while len(out_bin) % 4:
            out_bin.append(0)
        views.append({"buffer": 0, "byteOffset": len(out_bin), "byteLength": length})
        out_bin.extend(blob[start : start + length])
        accessor["bufferView"] = len(views) - 1
        accessors.append(accessor)
        return len(accessors) - 1

    skin = dict(document["skins"][0])
    skin["inverseBindMatrices"] = copy_accessor(skin["inverseBindMatrices"])
    nodes = [{key: value for key, value in node.items() if key not in ("mesh", "skin")} for node in document["nodes"]]
    clips = []
    for clip in document["animations"]:
        if clip["name"] not in PACKED:
            continue
        samplers = [{"input": copy_accessor(s["input"]), "output": copy_accessor(s["output"]),
                     "interpolation": s.get("interpolation", "LINEAR")} for s in clip["samplers"]]
        clips.append({"name": clip["name"], "samplers": samplers, "channels": clip["channels"]})
    pack = {
        "asset": {"version": "2.0", "generator": "make_arms.py"},
        "scene": 0,
        "scenes": document["scenes"],
        "nodes": nodes,
        "skins": [skin],
        "animations": clips,
        "accessors": accessors,
        "bufferViews": views,
    }
    write_glb(pack_path, pack, out_bin)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
