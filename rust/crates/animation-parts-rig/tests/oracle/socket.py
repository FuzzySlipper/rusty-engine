"""Writes socket.png: a cream ellipse, 160x96, soft-edged alpha, transparent outside."""
import struct, sys, zlib
w, h = 160, 96
rows = []
for y in range(h):
    row = bytearray([0])
    for x in range(w):
        dx, dy = (x + 0.5 - w / 2) / (w / 2 - 2), (y + 0.5 - h / 2) / (h / 2 - 2)
        r = (dx * dx + dy * dy) ** 0.5
        a = max(0.0, min(1.0, (1.0 - r) * 30))
        row += bytes([245, 235, 215, round(a * 255)])
    rows.append(bytes(row))
raw = zlib.compress(b"".join(rows), 9)
def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", raw) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
