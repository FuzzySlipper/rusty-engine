"""A subscriber that stops reading, while another client measures input latency."""
import socket, subprocess, sys, time, json
port=int(sys.argv[1]); P=sys.argv[2]
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
s.connect(("127.0.0.1",port))
s.sendall(f"GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/event-stream\r\n\r\n".encode())
time.sleep(0.2); s.recv(2048)  # read a little, then stall
started=time.perf_counter()
lat=subprocess.run([sys.executable, f"{P}/latency.py", str(port), "150", P], capture_output=True, text=True, timeout=120).stdout.strip()
stalled=time.perf_counter()-started
# Did the host give up on the stalled subscriber?
s.setblocking(False); drained=0; closed=False
try:
    while True:
        c=s.recv(1<<20)
        if not c: closed=True; break
        drained+=len(c)
except BlockingIOError: pass
print(json.dumps({"stalledSeconds":round(stalled,1),"otherClient":json.loads(lat),"stalledSubscriberClosedByHost":closed,"bufferedBytes":drained}))
