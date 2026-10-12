"""Deterministic fixture star catalogue and Milky Way band; no runtime generation."""
import json, math, struct, zlib
from pathlib import Path

CONTENT = Path(__file__).parent / 'content'
STARS = 3000
WIDTH, HEIGHT = 256, 128

def catalogue():
    state = 0x2545F491
    def next_value():
        nonlocal state
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        return state / 0xFFFFFFFF
    # The pole star at the catalogue's +Y, then stars over the sphere, more of them faint.
    stars = [[0, 1, 0, 2.0, 1, 0.95, 0.85]]
    for _ in range(STARS):
        z = next_value() * 2 - 1
        angle = next_value() * math.tau
        ring = math.sqrt(1 - z * z)
        magnitude = -1 + 7.5 * math.sqrt(next_value())
        tone = next_value()
        color = [0.75, 0.85, 1.0] if tone < 0.2 else [1.0, 0.8, 0.6] if tone > 0.8 else [1.0, 0.97, 0.9]
        stars.append([round(ring * math.cos(angle), 5), round(z, 5), round(ring * math.sin(angle), 5), round(magnitude, 2), *color])
    return {'format': 'rusty.stars.v1', 'stars': stars}

def band():
    rows = []
    for y in range(HEIGHT):
        row = bytearray()
        for x in range(WIDTH):
            distance = abs(y - HEIGHT / 2 + 8 * math.sin(x * 0.05))
            value = int(40 * math.exp(-distance * distance / 60))
            row.extend([value, value, min(255, int(value * 1.2)), 255])
        rows.append(b'\0' + row)
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xFFFFFFFF)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', WIDTH, HEIGHT, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b''.join(rows))) + chunk(b'IEND', b'')

(CONTENT / 'sky.stars.json').write_text(json.dumps(catalogue(), separators=(',', ':')))
(CONTENT / 'band.png').write_bytes(band())
