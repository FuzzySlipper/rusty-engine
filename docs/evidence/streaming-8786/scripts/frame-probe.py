#!/usr/bin/env python3
"""Pull frames from a runtime's frame route and report what arrives.

Usage: frame-probe.py <origin> <width> <height> <seconds> [save.jpg]
Requests one frame at a time, as the browser shell does, and prints frames/s,
payload bytes/s, median frame size and the gap between frames. Saves the
last frame's payload when asked.
"""
import http.client, json, statistics, struct, sys, time, urllib.parse

origin, width, height, seconds = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4])
save = sys.argv[5] if len(sys.argv) > 5 else None
url = urllib.parse.urlparse(origin)
frames, arrivals, after, last = [], [], 0, b""
started = time.monotonic()
while time.monotonic() - started < seconds:
    conn = http.client.HTTPConnection(url.hostname, url.port, timeout=10)
    conn.request("GET", f"/__rusty/product/runtime/frames?after={after}&width={width}&height={height}")
    response = conn.getresponse()
    body = response.read()
    conn.close()
    if response.status == 204:
        continue
    assert response.status == 200, response.status
    magic, header_len, sequence, step, w, h, fmt, flags, _, payload_len = struct.unpack("<4sIQQIIBBHI", body[:40])
    assert magic == b"RSF1"
    last = body[header_len:header_len + payload_len]
    arrivals.append(time.monotonic())
    frames.append({"sequence": sequence, "step": step, "size": [w, h], "format": fmt, "held": bool(flags & 1), "bytes": payload_len})
    after = sequence
span = arrivals[-1] - arrivals[0] if len(arrivals) > 1 else 0
gaps = [1000 * (b - a) for a, b in zip(arrivals, arrivals[1:])]
print(json.dumps({
    "frames": len(frames),
    "framesPerSecond": round((len(frames) - 1) / span, 1) if span else None,
    "payloadBytesPerSecond": round(sum(f["bytes"] for f in frames[1:]) / span) if span else None,
    "medianFrameBytes": statistics.median(f["bytes"] for f in frames) if frames else None,
    "medianGapMs": round(statistics.median(gaps), 2) if gaps else None,
    "p95GapMs": round(sorted(gaps)[int(len(gaps) * 0.95)], 2) if gaps else None,
    "skippedSequences": frames[-1]["sequence"] - frames[0]["sequence"] + 1 - len(frames) if frames else None,
    "first": frames[0] if frames else None, "last": frames[-1] if frames else None,
}, indent=1))
if save and last:
    open(save, "wb").write(last)
