#!/usr/bin/env python3
"""Finds where Doom's sound clips occur in a null-sink monitor recording.

Usage: match-clips.py RECORDING.wav CLIP.wav [CLIP.wav ...]

Each clip is resampled to the recording's rate and slid over the mono
recording. The score at each offset is the normalized correlation between the
clip and that stretch of recording (1.0 is an exact scaled copy). Prints, per
clip, the best score and every non-overlapping window scoring above the
threshold, with its start time. Clips the run never played serve as controls.
"""

import sys
import wave

import numpy as np

THRESHOLD = 0.6


def read(path):
    with wave.open(path) as source:
        rate = source.getframerate()
        channels = source.getnchannels()
        width = source.getsampwidth()
        frames = source.readframes(source.getnframes())
    dtype = {1: np.uint8, 2: "<i2"}[width]
    samples = np.frombuffer(frames, dtype=dtype).astype(np.float64)
    if width == 1:
        samples -= 128
    return samples.reshape(-1, channels).mean(axis=1), rate


def normalized_correlation(recording, clip):
    n = len(clip)
    clip = clip - clip.mean()
    clip /= np.linalg.norm(clip) or 1.0
    size = 1 << int(np.ceil(np.log2(len(recording) + n)))
    raw = np.fft.irfft(np.fft.rfft(recording, size) * np.conj(np.fft.rfft(clip, size)), size)[: len(recording) - n + 1]
    cumulative = np.concatenate([[0.0], np.cumsum(recording)])
    cumulative_sq = np.concatenate([[0.0], np.cumsum(recording * recording)])
    window_sum = cumulative[n:] - cumulative[:-n]
    window_energy = cumulative_sq[n:] - cumulative_sq[:-n] - window_sum * window_sum / n
    return raw / np.sqrt(np.maximum(window_energy, 1e-12))


recording, rate = read(sys.argv[1])
print(f"recording {len(recording) / rate:.2f} s at {rate} Hz")
for path in sys.argv[2:]:
    clip, clip_rate = read(path)
    positions = np.arange(0, len(clip) - 1, clip_rate / rate)
    resampled = np.interp(positions, np.arange(len(clip)), clip)
    scores = normalized_correlation(recording, resampled)
    hits = []
    order = np.argsort(scores)[::-1]
    for index in order:
        if scores[index] < THRESHOLD:
            break
        if all(abs(index - other) > len(resampled) for other, _ in hits):
            hits.append((index, scores[index]))
    hits.sort()
    name = path.rsplit("/", 1)[-1]
    listed = ", ".join(f"{index / rate:.2f} s ({score:.3f})" for index, score in hits) or "none"
    print(f"{name}: best {scores.max():.3f}; above {THRESHOLD}: {len(hits)}: {listed}")
