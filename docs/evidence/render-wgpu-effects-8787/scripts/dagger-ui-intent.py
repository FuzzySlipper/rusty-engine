#!/usr/bin/env python3
"""Post dagger.ui payload intents to a running product.

usage: intent.py <origin> <action> [<action> ...]
Takes the runtime binding and next input sequence from a fresh attachment.
"""
import json
import re
import sys
import time
import urllib.request

origin, actions = sys.argv[1], sys.argv[2:]
headers = {"Origin": origin, "Accept": "text/event-stream"}


def binding_and_sequence():
    request = urllib.request.Request(f"{origin}/__rusty/product/runtime/outputs/fresh", headers=headers)
    runtime, sequence, text = None, None, ""
    deadline = time.time() + 20
    with urllib.request.urlopen(request, timeout=30) as stream:
        for raw in stream:
            text += raw.decode().replace('\\"', '"')
            if runtime is None:
                match = re.search(r'"kind":"binding","runtime":(\{[^}]*\})', text)
                if match:
                    runtime = json.loads(match.group(1))
            matches = re.findall(r'"nextInputSequence":"?(\d+)', text)
            if matches:
                sequence = int(matches[-1])
            if runtime is not None and sequence is not None:
                return runtime, sequence
            if time.time() > deadline:
                break
    raise SystemExit(f"no binding/sequence: runtime={runtime} sequence={sequence}")


for action in actions:
    runtime, sequence = binding_and_sequence()
    body = {"batch": [{
        "runtime": runtime,
        "sequence": str(sequence),
        "context": "gameplay.default",
        "intent": "dagger.ui",
        "value": {"kind": "product-payload", "contract": "dagger.ui.action.v1", "data": {"action": action}},
    }]}
    request = urllib.request.Request(
        f"{origin}/__rusty/product/runtime/input",
        data=json.dumps(body).encode(),
        headers={"Origin": origin, "Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        print(action, response.status, response.read().decode()[:300])
    time.sleep(1.5)
