"""Writes content/grass.png and content/rock.png, 64x64 tileable noise for the fixture's two terrain layers,
content/marks.png, a 64x64 decal atlas of four 32x32 marks: a footprint, a scorch, a splash and an ash ring,
and content/coals.png with content/coals-emission.png, a 64x64 atlas of four 32x32 patches of char crossed by
glowing seams and single-texel coals, and the light they emit laid out the same."""
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


def marks():
    image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    half = SIZE // 2
    for y in range(half):
        for x in range(half):
            u, v = (x + 0.5) / half * 2 - 1, (y + 0.5) / half * 2 - 1
            # A footprint: a sole and a heel, toes up the region.
            sole = (u / 0.45) ** 2 + ((v + 0.25) / 0.6) ** 2 < 1
            heel = (u / 0.38) ** 2 + ((v - 0.6) / 0.3) ** 2 < 1
            if sole or heel:
                image.putpixel((x, y), (40, 34, 28, 210))
            # A scorch: dark at the heart, thinning out.
            r = math.hypot(u, v)
            if r < 1:
                image.putpixel((half + x, y), (20, 16, 12, int(255 * (1 - r) ** 0.7)))
            # A splash: a disc with lobes.
            lobes = 0.55 + 0.25 * math.cos(5 * math.atan2(v, u))
            if r < lobes:
                image.putpixel((x, half + y), (120, 10, 8, 230))
            # An ash ring.
            if 0.55 < r < 0.85:
                image.putpixel((half + x, half + y), (90, 88, 84, 220))
    return image


marks().save(content / "marks.png")


def coals():
    """Char with seams and single-texel coals, and the light they emit."""
    color = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    emission = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 255))
    half = SIZE // 2
    for patch in range(4):
        rng = random.Random(10396 + patch)
        left, top = patch % 2 * half, patch // 2 * half
        # A seam wanders across the patch; coals lie scattered.
        seam = {(x, int(half / 2 + 6 * math.sin(x / 4 + patch))) for x in range(half)}
        for y in range(half):
            for x in range(half):
                u, v = (x + 0.5) / half * 2 - 1, (y + 0.5) / half * 2 - 1
                if math.hypot(u, v) >= 1:
                    continue
                pixel, glow = (22, 18, 16, 235), (0, 0, 0)
                if (x, y) in seam:
                    pixel, glow = (150, 60, 20, 255), (200, 70, 15)
                elif rng.random() < 0.04:
                    pixel, glow = (220, 110, 30, 255), (255, 140, 40)
                color.putpixel((left + x, top + y), pixel)
                emission.putpixel((left + x, top + y), glow + (255,))
    return color, emission


coal_color, coal_emission = coals()
coal_color.save(content / "coals.png")
coal_emission.save(content / "coals-emission.png")
