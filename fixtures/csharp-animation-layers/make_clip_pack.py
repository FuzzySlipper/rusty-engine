#!/usr/bin/env python3
"""Writes the joint-attachment character's clip pack, body.clips.glb.

The pack carries only the character's skeleton (its nodes and skin) and
three clips, no mesh: `wave`, the right arm raised and lowered; `nod`, the
head dipped and raised, made to be laid additively; and `leap`, the
character's own jump. A development tool: the pack is committed, and a
product never runs this.

    python3 make_clip_pack.py ../csharp-joint-attachments/content/body.glb \\
        ../csharp-joint-attachments/content/body.clips.glb
"""

import json
import math
import struct
import sys


def read_glb(path):
    data = open(path, "rb").read()
    json_length = struct.unpack_from("<I", data, 12)[0]
    document = json.loads(data[20 : 20 + json_length])
    bin_offset = 20 + json_length
    bin_length = struct.unpack_from("<I", data, bin_offset)[0]
    return document, data[bin_offset + 8 : bin_offset + 8 + bin_length]


def quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]


def axis_angle(axis, angle):
    s = math.sin(angle / 2)
    return [axis[0] * s, axis[1] * s, axis[2] * s, math.cos(angle / 2)]


def main(source, target):
    document, blob = read_glb(source)
    names = {node.get("name"): index for index, node in enumerate(document["nodes"])}
    out_bin = bytearray()
    views, accessors = [], []

    def add(data, accessor):
        while len(out_bin) % 4:
            out_bin.append(0)
        views.append({"buffer": 0, "byteOffset": len(out_bin), "byteLength": len(data)})
        out_bin.extend(data)
        accessor["bufferView"] = len(views) - 1
        accessors.append(accessor)
        return len(accessors) - 1

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
        return add(blob[start : start + length], accessor)

    def floats(values):
        return struct.pack(f"<{len(values)}f", *values)

    def rotation_clip(name, keys, joints):
        times = [time for time, _ in keys]
        time_accessor = add(
            floats(times),
            {"componentType": 5126, "count": len(times), "type": "SCALAR",
             "min": [times[0]], "max": [times[-1]]},
        )
        samplers, channels = [], []
        for joint, axis in joints:
            rest = document["nodes"][names[joint]].get("rotation", [0, 0, 0, 1])
            values = []
            for _, angle in keys:
                values += quat_mul(rest, axis_angle(axis, angle))
            output = add(floats(values), {"componentType": 5126, "count": len(keys), "type": "VEC4"})
            samplers.append({"input": time_accessor, "output": output})
            channels.append({"sampler": len(samplers) - 1,
                             "target": {"node": names[joint], "path": "rotation"}})
        return {"name": name, "samplers": samplers, "channels": channels}

    skin = dict(document["skins"][0])
    skin["inverseBindMatrices"] = copy_accessor(skin["inverseBindMatrices"])
    nodes = [{key: value for key, value in node.items() if key not in ("mesh", "skin")}
             for node in document["nodes"]]
    wave = rotation_clip(
        "wave",
        [(0.0, 0.0), (0.5, -1.4), (1.0, -1.1), (1.5, -1.4), (2.0, 0.0)],
        [("RightArm", (0, 0, 1)), ("RightForeArm", (0, 0, 1))],
    )
    nod = rotation_clip("nod", [(0.0, 0.0), (0.4, 0.35), (0.8, 0.0)], [("Head", (1, 0, 0))])
    jump = next(clip for clip in document["animations"] if clip["name"] == "jump")
    leap_samplers = []
    for sampler in jump["samplers"]:
        leap_samplers.append({
            "input": copy_accessor(sampler["input"]),
            "output": copy_accessor(sampler["output"]),
            "interpolation": sampler.get("interpolation", "LINEAR"),
        })
    leap = {"name": "leap", "samplers": leap_samplers, "channels": jump["channels"]}
    while len(out_bin) % 4:
        out_bin.append(0)
    pack = {
        "asset": {"version": "2.0", "generator": "make_clip_pack.py"},
        "scene": 0,
        "scenes": document["scenes"],
        "nodes": nodes,
        "skins": [skin],
        "animations": [wave, nod, leap],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": len(out_bin)}],
    }
    text = json.dumps(pack, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    total = 12 + 8 + len(text) + 8 + len(out_bin)
    with open(target, "wb") as out:
        out.write(b"glTF" + struct.pack("<II", 2, total))
        out.write(struct.pack("<I", len(text)) + b"JSON" + text)
        out.write(struct.pack("<I", len(out_bin)) + b"BIN\0" + bytes(out_bin))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
