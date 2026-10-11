"""Writes content/grass.png and content/rock.png: 64x64 tileable noise for the fixture's two terrain layers."""
import math
import random
from pathlib import Path

from PIL import Image

SIZE = 64


def tile(base, spread, seed):
    rng = random.Random(seed)
    lattice = [[rng.random() for _ in range(8)] for _ in range(8)]
    image = Image.new("RGBA", (SIZE, SIZE))
    for y in range(SIZE):
        for x in range(SIZE):
            u, v = x / SIZE * 8, y / SIZE * 8
            i, j = int(u), int(v)
            fu, fv = u - i, v - j
            a, b = lattice[j % 8][i % 8], lattice[j % 8][(i + 1) % 8]
            c, d = lattice[(j + 1) % 8][i % 8], lattice[(j + 1) % 8][(i + 1) % 8]
            smooth = lambda t: t * t * (3 - 2 * t)
            value = (a + (b - a) * smooth(fu)) * (1 - smooth(fv)) + (c + (d - c) * smooth(fu)) * smooth(fv)
            grain = rng.random() * 0.3
            shade = 1 + spread * (value + grain - 0.65)
            image.putpixel((x, y), tuple(max(0, min(255, int(channel * shade))) for channel in base) + (255,))
    return image


content = Path(__file__).resolve().parent / "content"
tile((92, 140, 58), 0.6, 9842).save(content / "grass.png")
tile((128, 122, 112), 0.9, 9843).save(content / "rock.png")
