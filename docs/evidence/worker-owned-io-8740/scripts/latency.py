"""Input->output latency through the browser transport (no browser)."""
import json, random, statistics, sys, threading, time
sys.path.insert(0, sys.argv[3])
from probe import sse, post
port=int(sys.argv[1]); count=int(sys.argv[2]); host="127.0.0.1"
stop=threading.Event(); consumed={}; lock=threading.Lock(); binding=[None,None]; sse_bytes=[0]
def on(now,ev,data,eid,size):
    sse_bytes[0]+=size
    if ev is not None or not data: return
    for o in json.loads(data).get("outputs",[]):
        if o["kind"]=="binding": binding[0]=o["runtime"]; binding[1]=int(o["nextInputSequence"])
        if o["kind"]=="runtime-input-result":
            with lock: consumed[int(o["result"]["consumedThrough"])]=now
threading.Thread(target=sse,args=(host,port,"/__rusty/product/runtime/outputs/fresh",on,stop),daemon=True).start()
while binding[0] is None: time.sleep(0.05)
time.sleep(0.5)
seq=binding[1]; rtt=[]; e2e=[]; start_bytes=sse_bytes[0]; t_start=time.perf_counter()
for i in range(count):
    edge="pressed" if i%2==0 else "released"
    body={"batch":[{"runtime":binding[0],"sequence":str(seq),"context":"gameplay.default","fact":{"kind":"key","code":"key-w","edge":edge}}]}
    t0=time.perf_counter(); head,_=post(host,port,"/__rusty/product/runtime/input",body); t1=time.perf_counter()
    assert " 200 " in head.split("\r\n")[0], head
    rtt.append((t1-t0)*1000)
    deadline=t0+2
    while time.perf_counter()<deadline:
        with lock:
            hit=[t for s,t in consumed.items() if s>=seq]
        if hit: e2e.append((min(hit)-t0)*1000); break
        time.sleep(0.0005)
    seq+=1
    time.sleep(random.uniform(0.02,0.04))
elapsed=time.perf_counter()-t_start
stop.set()
def pct(v,p): v=sorted(v); return v[min(len(v)-1,int(len(v)*p))]
print(json.dumps({"inputs":count,"postRttMs":{"p50":round(pct(rtt,.5),2),"p95":round(pct(rtt,.95),2)},
  "inputToResultMs":{"n":len(e2e),"p50":round(pct(e2e,.5),2),"p95":round(pct(e2e,.95),2),"max":round(max(e2e),2)},
  "sseBytesPerSec":round((sse_bytes[0]-start_bytes)/elapsed)}))
