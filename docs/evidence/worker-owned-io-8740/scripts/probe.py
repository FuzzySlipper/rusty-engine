"""Scripted browser-transport probe: SSE outputs + input POSTs, no browser."""
import json, socket, sys, threading, time

def sse(host, port, path, on_event, stop):
    s = socket.create_connection((host, port))
    s.sendall(f"GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: text/event-stream\r\n\r\n".encode())
    buf = b""
    # headers
    while b"\r\n\r\n" not in buf:
        buf += s.recv(65536)
    head, buf = buf.split(b"\r\n\r\n", 1)
    status = head.split(b"\r\n")[0]
    if b"200" not in status:
        print(f"SSE failed: {head!r} {buf[:500]!r}", flush=True); return
    s.settimeout(0.5)
    while not stop.is_set():
        while b"\n\n" in buf:
            raw, buf = buf.split(b"\n\n", 1)
            now = time.perf_counter()
            event, data, eid = None, [], None
            for line in raw.split(b"\n"):
                if line.startswith(b"event: "): event = line[7:].decode()
                elif line.startswith(b"data: "): data.append(line[6:])
                elif line.startswith(b"id: "): eid = line[4:].decode()
            on_event(now, event, b"\n".join(data), eid, len(raw) + 2)
        try:
            chunk = s.recv(1 << 20)
        except socket.timeout:
            continue
        if not chunk:
            on_event(time.perf_counter(), "closed", b"", None, 0); return
        buf += chunk

def post(host, port, path, body):
    s = socket.create_connection((host, port))
    b = json.dumps(body).encode()
    s.sendall(f"POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {len(b)}\r\nConnection: close\r\n\r\n".encode() + b)
    out = b""
    while True:
        c = s.recv(65536)
        if not c: break
        out += c
    s.close()
    head, _, payload = out.partition(b"\r\n\r\n")
    return head.decode(errors="replace"), payload

if __name__ == "__main__":
    host, port = "127.0.0.1", int(sys.argv[1])
    stop = threading.Event()
    events = []
    t = threading.Thread(target=sse, args=(host, port, "/__rusty/product/runtime/outputs/fresh", lambda *e: events.append(e), stop), daemon=True)
    t.start()
    time.sleep(float(sys.argv[2]) if len(sys.argv) > 2 else 3)
    stop.set()
    kinds = {}
    for now, ev, data, eid, size in events:
        k = ev or "data"
        if ev is None and data:
            try:
                d = json.loads(data)
                k = d.get("kind", "?") + ":" + ",".join(o.get("kind","?") for o in d.get("outputs", []))
            except Exception:
                pass
        kinds.setdefault(k, [0, 0]); kinds[k][0] += 1; kinds[k][1] += size
    for k, (n, b) in sorted(kinds.items(), key=lambda x: -x[1][1]):
        print(f"{n:5d} events {b:9d} bytes  {k[:150]}")
