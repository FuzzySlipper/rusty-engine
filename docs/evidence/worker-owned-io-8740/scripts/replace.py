"""Worker replacement as an EventSource client experiences it."""
import json, os, sys, threading, time
sys.path.insert(0, sys.argv[3])
from probe import sse
port=int(sys.argv[1]); trigger=sys.argv[2]; host="127.0.0.1"
def run(path, until):
    events=[]; stop=threading.Event()
    def on(now,ev,data,eid,size):
        events.append((now,ev,data,eid))
        if until(ev,data): stop.set()
    t=threading.Thread(target=sse,args=(host,port,path,on,stop),daemon=True); t.start()
    t.join(timeout=60); stop.set(); return events
def instance(data):
    try:
        for o in json.loads(data).get("outputs",[]):
            if o["kind"]=="binding": return o["runtime"]["instanceId"]
    except Exception: return None
first=[]
def until_closed(ev,data): return ev=="closed"
# Attach, then trigger replacement once the baseline arrived.
def trig():
    time.sleep(1.0); open(trigger,"w").close(); first.append(time.perf_counter())
threading.Thread(target=trig,daemon=True).start()
a=run("/__rusty/product/runtime/outputs/fresh", until_closed)
old=[instance(d) for _,e,d,_ in a if e is None and d and instance(d)][0]
last_id=[i for _,_,_,i in a if i][-1]
closed=a[-1][0]
# EventSource retries with Last-Event-ID against the same origin.
import socket
s=socket.create_connection((host,port)); s.sendall(f"GET /__rusty/product/runtime/outputs HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: text/event-stream\r\nLast-Event-ID: {last_id}\r\n\r\n".encode())
resume=b""; s.settimeout(5)
while b"\n\n" not in resume.split(b"\r\n\r\n",1)[-1] or b"\r\n\r\n" not in resume: resume+=s.recv(4096)
s.close()
b=run("/__rusty/product/runtime/outputs/fresh", lambda ev,data: ev=="rusty-output-baseline")
new=[instance(d) for _,e,d,_ in b if e is None and d and instance(d)][0]
print(json.dumps({"oldInstance":old,"newInstance":new,
  "streamClosedAfterTriggerMs":round((closed-first[0])*1000),
  "resumeWithOldCursor":resume.split(b"\r\n\r\n",1)[1].decode()[:120],
  "freshBaselineAfterTriggerMs":round((b[-1][0]-first[0])*1000)}))
