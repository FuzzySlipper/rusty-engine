"""Linux packaged voxel-fixture regression for terminal-style signal delivery.

python3 group-signal-proof.py HOST PRODUCT OUTPUT_DIRECTORY
Uses a fresh session so killpg cannot signal the calling agent or terminal.
"""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

host, product, output = sys.argv[1:]
output = Path(output)
output.mkdir(parents=True, exist_ok=True)
results = []
for supervised in (False, True):
    for sig in (signal.SIGINT, signal.SIGTERM):
        name = ("supervised-" if supervised else "direct-") + sig.name
        stem = output / name
        diagnostics = stem.with_suffix(".ndjson")
        diagnostics.unlink(missing_ok=True)
        env = dict(os.environ, RUSTY_ENGINE_DIAGNOSTICS_PATH=str(diagnostics))
        command = [host, "--product", product, "--loader", "coreclr"]
        if supervised:
            command.append("--supervised")
        with stem.with_suffix(".log").open("w") as log:
            process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=log,
                                       stderr=subprocess.STDOUT, env=env,
                                       start_new_session=True)
            try:
                deadline = time.monotonic() + 30
                while process.poll() is None and time.monotonic() < deadline:
                    if "VOXEL_PROOF updates=" in stem.with_suffix(".log").read_text():
                        break
                    time.sleep(0.1)
                else:
                    raise RuntimeError(f"{name}: fixture did not update")
                children = Path(f"/proc/{process.pid}/task/{process.pid}/children").read_text().split()
                assert len(children) == 1, children
                worker_pid = int(children[0])
                shell_group = os.getpgid(process.pid)
                worker_group = os.getpgid(worker_pid)
                assert shell_group == process.pid and worker_group != shell_group
                started = time.monotonic()
                os.killpg(shell_group, sig)
                exit_code = process.wait(timeout=20)
                elapsed = time.monotonic() - started
                events = [json.loads(line) for line in diagnostics.read_text().splitlines()]
                disposed = sum(event.get("code") == "DISPOSED" for event in events)
                result = dict(case=name, exit_code=exit_code, disposed=disposed,
                              elapsed_seconds=elapsed, shell_group=shell_group,
                              worker_group=worker_group,
                              worker_reaped=not Path(f"/proc/{worker_pid}").exists())
                results.append(result)
                (output / "result.json").write_text(json.dumps(results, indent=2))
                assert exit_code == 0 and disposed == 1 and result["worker_reaped"], result
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
                process.stdin.close()
print(json.dumps(results, indent=2))
