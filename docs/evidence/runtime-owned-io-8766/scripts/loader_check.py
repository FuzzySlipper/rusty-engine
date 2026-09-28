import json, os, subprocess, sys, time, urllib.request, signal
# Usage: loader_check.py <runtime-pack rusty-product-host> <Product with both
# artifacts> <NativeAOT-only Product copy> <output.json>
# Products come from `RUSTY_ENGINE_SDK_TEST_KEEP_WORK=1
# scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot`; the NativeAOT-only
# copy drops `coreclr/` and the manifest's `coreclr` entry.
HOST, BOTH, AOT_ONLY, OUTPUT = sys.argv[1:5]
URL = "http://127.0.0.1:40821"

def children(pid):
    out = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout.split()
    return [int(p) for p in out]

def mapped(pid):
    try:
        maps = open(f"/proc/{pid}/maps").read()
    except OSError:
        return None
    return {"nativeAotModule": "Rusty.Engine.Product.so" in maps, "coreclr": "libcoreclr.so" in maps}

def case(name, product, loader, extra, env=None):
    result = {"case": name, "product": os.path.basename(product), "loader": loader, "extra": extra}
    proc = subprocess.Popen([HOST, "--product", product, "--loader", loader, *extra],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, env={**os.environ, **(env or {})}, start_new_session=True)
    status = None
    deadline = time.time() + 60
    while time.time() < deadline:
        if proc.poll() is not None:
            break
        try:
            with urllib.request.urlopen(f"{URL}/", timeout=2) as r:
                status = r.status
                break
        except Exception as e:
            time.sleep(0.25)
    result["httpStatus"] = status
    runtime = [c for c in children(proc.pid) if "--serve-listener-fd" in open(f"/proc/{c}/cmdline").read().replace("\0", " ")]
    if runtime:
        cmd = open(f"/proc/{runtime[0]}/cmdline").read().split("\0")
        result["runtimeLoaderArg"] = cmd[cmd.index("--loader") + 1]
        result["runtimeMapped"] = mapped(runtime[0])
    else:
        result["runtimeLoaderArg"] = None
    started = time.time()
    if proc.poll() is None:
        proc.stdin.close() if "--supervised" in extra else os.killpg(proc.pid, signal.SIGINT)
    try:
        proc.wait(timeout=20)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL); proc.wait()
    result["exit"] = proc.returncode
    result["stopSeconds"] = round(time.time() - started, 3)
    out = proc.stdout.read()
    result["disposed"] = out.count("DISPOSED")
    result["errors"] = [l for l in out.splitlines() if "coreclr" in l.lower() and ("required" in l or "error" in l.lower())][:3]
    result["tail"] = out.splitlines()[-3:]
    return result

chromium = os.environ.get("RUSTY_CHROMIUM_PATH")
results = [
    case("nativeaot-only, supervised", AOT_ONLY, "nativeaot", ["--supervised"]),
    case("both artifacts, nativeaot, supervised", BOTH, "nativeaot", ["--supervised"]),
    case("both artifacts, coreclr, supervised (control)", BOTH, "coreclr", ["--supervised"]),
    case("nativeaot-only, direct (unsupervised, in process)", AOT_ONLY, "nativeaot", []),
]
if chromium:
    results.append(case("nativeaot-only, headless", AOT_ONLY, "nativeaot", ["--headless"]))
json.dump(results, open(OUTPUT, "w"), indent=2)
print(json.dumps(results, indent=2))
