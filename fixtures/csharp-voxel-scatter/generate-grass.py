"""Writes content/grass.png: blades of grass on a transparent card.

Blades rise from the bottom edge, tapering to a point, darker at the root and
lighter toward the tip. Alpha is 1 inside a blade and 0 outside, for a masked
material. Run from this directory: python3 generate-grass.py
"""
import math
import random

from PIL import Image

SIZE = 128
BLADES = 22
ROOT = (52, 88, 30)
TIP = (150, 186, 82)

random.seed(9546)
image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
pixels = image.load()
for _ in range(BLADES):
    base = random.uniform(6, SIZE - 6)
    height = random.uniform(0.55, 0.98) * SIZE
    width = random.uniform(3.0, 6.0)
    lean = random.uniform(-0.35, 0.35)
    for y in range(SIZE):
        rise = SIZE - 1 - y
        if rise > height:
            continue
        t = rise / height
        centre = base + lean * rise + 6 * math.sin(t * 2.2) * lean
        half = width * (1 - t) * 0.5 + 0.4
        shade = [round(r + (p - r) * t) for r, p in zip(ROOT, TIP)]
        for x in range(int(centre - half), int(centre + half) + 1):
            if 0 <= x < SIZE:
                pixels[x, y] = (*shade, 255)
image.save("content/grass.png")
