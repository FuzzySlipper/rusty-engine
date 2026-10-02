#!/usr/bin/env bash
# Packs the two content modules ContentContainerChecks.cs opens into a module
# library: `srd` (a ruleset that requires `walls`) packed raw, and `walls` (a
# portrait and an animated GLB whose image is a separate file beside it)
# packed compressed. A text file beside them is not a container, and
# `broken.container` holds an entry that no longer decompresses.
#
# usage: make-content-modules.sh <library-dir> <rusty command...>
set -euo pipefail

library=$1
shift
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
sources=$(mktemp -d "${TMPDIR:-/tmp}/rusty-content-modules.XXXXXX")
trap 'rm -rf -- "$sources"' EXIT

mkdir -p "$library" "$sources/srd/rules" "$sources/walls/portraits" "$sources/walls/models"
printf '{"id":"srd","requires":["walls"]}' > "$sources/srd/module.json"
printf '{"fighter":{"portrait":"portraits/fighter.png","model":"models/character.glb"}}' \
    > "$sources/srd/rules/classes.json"
printf '{"id":"walls"}' > "$sources/walls/module.json"
cp "$repo_root/fixtures/csharp-nativeaot-trial/content/trial.png" "$sources/walls/portraits/fighter.png"
# Move the GLB's embedded image out to skin.png, referenced by a relative URI.
python3 - "$repo_root/fixtures/render/assets/kenney-retro-character/character-medium.glb" \
    "$sources/walls/models" <<'PY'
import json
import struct
import sys

source_path, directory = sys.argv[1:]
source = open(source_path, "rb").read()
json_length = struct.unpack_from("<I", source, 12)[0]
document = json.loads(source[20:20 + json_length])
binary = source[20 + json_length + 8:]
image = document["images"][0]
view = document["bufferViews"][image.pop("bufferView")]
image.pop("mimeType", None)
offset = view.get("byteOffset", 0)
open(f"{directory}/skin.png", "wb").write(binary[offset:offset + view["byteLength"]])
image["uri"] = "skin.png"
encoded = json.dumps(document).encode("utf-8")
encoded += b" " * (-len(encoded) % 4)
rest = source[20 + json_length:]
body = struct.pack("<II", len(encoded), 0x4E4F534A) + encoded + rest
open(f"{directory}/character.glb", "wb").write(struct.pack("<4sII", b"glTF", 2, 12 + len(body)) + body)
PY
"$@" pack-content "$sources/srd" --output "$library/srd.rpak"
"$@" pack-content "$sources/walls" --output "$library/walls.rpak" --compress
printf 'not a container' > "$library/notes.txt"
# A container whose compressed entry no longer decompresses: header and
# inventory stay valid, so it opens and refuses only when the entry is read.
mkdir -p "$sources/broken/data"
printf '{"id":"broken"}' > "$sources/broken/module.json"
python3 -c 'print("[1,2,3]," * 4096)' > "$sources/broken/data/table.json"
"$@" pack-content "$sources/broken" --output "$library/broken.container" --compress
python3 - "$library/broken.container" <<'PY'
import json
import struct
import sys

path = sys.argv[1]
data = bytearray(open(path, "rb").read())
offset, length = struct.unpack_from("<QQ", data, 8)
entry = next(e for e in json.loads(data[offset:offset + length])["entries"] if e["path"] == "data/table.json")
assert "zstdLength" in entry, "the table must be stored compressed"
data[entry["offset"]:entry["offset"] + 4] = b"XXXX"
open(path, "wb").write(data)
PY
