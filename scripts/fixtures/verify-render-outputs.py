#!/usr/bin/env python3
"""Inspect actual packaged renderer output; no image editing or provider imports."""
import json
import struct
import sys
import zlib
from pathlib import Path
root = Path(sys.argv[1])


def unfilter(pixels, width, height):
    """Reverse PNG row filters (RGBA8). Rows keep a leading 0 filter byte, so
    channels stay at indices 1..=4 of each pixel as the checks below read them."""
    stride, rows, previous = width * 4, [], bytearray(width * 4)
    for i in range(height):
        start = i * (stride + 1)
        kind, line = pixels[start], bytearray(pixels[start + 1:start + 1 + stride])
        for x in range(stride):
            left = line[x - 4] if x >= 4 else 0
            up = previous[x]
            corner = previous[x - 4] if x >= 4 else 0
            if kind == 1:
                line[x] = (line[x] + left) & 255
            elif kind == 2:
                line[x] = (line[x] + up) & 255
            elif kind == 3:
                line[x] = (line[x] + (left + up) // 2) & 255
            elif kind == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                predictor = left if pa <= pb and pa <= pc else up if pb <= pc else corner
                line[x] = (line[x] + predictor) & 255
            else:
                assert kind == 0, f'unknown PNG filter {kind}'
        rows.append(bytes([0]) + bytes(line))
        previous = line
    return rows


decoded = {}
for path in root.glob('*.png'):
    data = path.read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n', path
    offset, compressed = 8, bytearray()
    while offset < len(data):
        count, = struct.unpack_from('>I', data, offset)
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + count]
        if kind == b'IHDR':
            width, height, bits, color, *_ = struct.unpack('>IIBBBBB', payload)
            assert (width, height, bits, color) == (96, 80, 8, 6), path
        if kind == b'IDAT': compressed.extend(payload)
        offset += count + 12
    pixels = zlib.decompress(compressed)
    rows = unfilter(pixels, width, height)
    alpha = [a for row in rows for a in row[4::4]]
    if path.name not in ('background.png', 'camera-background.png'):
        assert any(a == 0 for a in alpha) and any(a > 0 for a in alpha), f'{path}: expected transparent background and rendered geometry'
    decoded[path.name] = rows
    assert any(any(row[i:i+3]) for row in rows for i in range(1, len(row), 4)), f'{path}: geometry was black'
if 'selected-child.png' in decoded:
    coverage = lambda rows: sum(a > 0 for row in rows for a in row[4::4])
    assert coverage(decoded['selected-child.png']) < coverage(decoded['generated.png']) / 3, 'ancestor geometry leaked into selected image'
assert decoded['dim.png'] != decoded['generated.png'], 'exposure was ignored'
assert decoded['aces.png'] != decoded['generated.png'], 'ACES was ignored'
corner = decoded['background.png'][0][1:5]
assert all(abs(a - b) <= 2 for a, b in zip(corner, [137, 188, 225, 128])), f'background color/alpha conversion failed: {list(corner)}'
camera_corner = decoded['camera-background.png'][0][1:5]
assert all(abs(a - b) <= 2 for a, b in zip(camera_corner, [137, 188, 225, 255])), f'retained camera background failed: {list(camera_corner)}'
for path in root.glob('*.glb'):
    data = path.read_bytes()
    magic, version, length = struct.unpack_from('<4sII', data)
    assert (magic, version, length) == (b'glTF', 2, len(data)), path
    size, kind = struct.unpack_from('<I4s', data, 12)
    assert kind == b'JSON', path
    doc = json.loads(data[20:20 + size])
    assert doc.get('meshes') and doc.get('nodes') and doc.get('materials'), path
    for mesh in doc['meshes']:
        for primitive in mesh['primitives']:
            assert 'POSITION' in primitive['attributes'], path
            assert 'NORMAL' in primitive['attributes'], path
    if 'child' in path.name:
        assert len(doc['meshes']) == 1, 'unselected ancestor geometry leaked into export'
        assert any(n.get('children') for n in doc['nodes']), 'ancestor transform chain lost'
    if 'generated' in path.name:
        assert len(doc['meshes']) >= 2 and any(n.get('children') for n in doc['nodes']), 'multipart hierarchy lost'
        assert any('TEXCOORD_0' in p['attributes'] for m in doc['meshes'] for p in m['primitives']), path
    if 'animated' in path.name:
        assert doc.get('skins') and doc.get('animations'), 'skin or clips were silently dropped'
        assert any(a.get('name') == 'run' for a in doc['animations']), 'named run clip lost'
print(f'Validated renderer output in {root}')
