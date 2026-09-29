#!/usr/bin/env python3
"""Summarize a null-sink monitor recording: level per window and the
dominant frequency while sound is present. Usage: analyze-recording.py WAV"""

import sys
import wave

import numpy as np

WINDOW_SECONDS = 0.25
SILENCE_RMS = 1e-4

with wave.open(sys.argv[1]) as recording:
    rate = recording.getframerate()
    channels = recording.getnchannels()
    frames = recording.readframes(recording.getnframes())
samples = np.frombuffer(frames, dtype="<i2").reshape(-1, channels).mean(axis=1) / 32768.0
window = int(rate * WINDOW_SECONDS)
levels = []
for start in range(0, len(samples) - window + 1, window):
    chunk = samples[start : start + window]
    levels.append(float(np.sqrt(np.mean(chunk * chunk))))
audible = [index for index, level in enumerate(levels) if level > SILENCE_RMS]
print(f"duration {len(samples) / rate:.2f} s, {len(levels)} windows of {WINDOW_SECONDS} s")
print("rms per window:", " ".join(f"{level:.4f}" for level in levels))
if not audible:
    print("no audible window")
    sys.exit(1)
first, last = audible[0], audible[-1]
print(f"audible from {first * WINDOW_SECONDS:.2f} s to {(last + 1) * WINDOW_SECONDS:.2f} s")
sound = samples[first * window : (last + 1) * window]
spectrum = np.abs(np.fft.rfft(sound * np.hanning(len(sound))))
frequencies = np.fft.rfftfreq(len(sound), 1 / rate)
print(f"dominant frequency {frequencies[int(np.argmax(spectrum))]:.1f} Hz")
