"""Compare two RGB captures pixel by pixel.

    compare.py <a.png> <b.png>
"""

import sys

import numpy as np
from PIL import Image

a = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
b = np.asarray(Image.open(sys.argv[2]).convert("RGB")).astype(int)
if a.shape != b.shape:
    sys.exit(f"sizes differ: {a.shape} {b.shape}")
difference = np.abs(a - b)
channel = difference.max(axis=2)
print(
    f"{sys.argv[1]} vs {sys.argv[2]}: {a.shape[1]}x{a.shape[0]}, "
    f"{int((channel > 0).sum())} of {channel.size} pixels differ "
    f"({(channel > 0).mean() * 100:.4f}%), max {int(channel.max())}, "
    f"mean {difference.mean():.5f}"
)
