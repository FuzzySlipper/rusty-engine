#!/usr/bin/env python3
"""Check the expensive Studio/demo CI ownership and orchestration contract."""

from pathlib import Path
import re
import sys


REPO_ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = REPO_ROOT / ".github/workflows/studio-demo-integration.yml"
PARENT = REPO_ROOT / "scripts/verify-studio-demo-integration.sh"
BROWSER = REPO_ROOT / "scripts/verify-studio-browser-integration.sh"
ENTITY_INSPECTOR = REPO_ROOT / "scripts/verify-studio-entity-inspector-integration.sh"


def reject(message: str) -> None:
    print(f"studio demo CI policy violation: {message}", file=sys.stderr)
    raise SystemExit(1)


workflow = WORKFLOW.read_text(encoding="utf-8")
parent = PARENT.read_text(encoding="utf-8")
browser = BROWSER.read_text(encoding="utf-8")
entity_inspector = ENTITY_INSPECTOR.read_text(encoding="utf-8")
trigger_paths = re.findall(r"^\s+- '([^']+)'\s*$", workflow, flags=re.MULTILINE)
push_section = workflow.split("  push:\n", maxsplit=1)[1].split(
    "  pull_request:\n", maxsplit=1
)[0]
pull_request_section = workflow.split("  pull_request:\n", maxsplit=1)[1].split(
    "  workflow_dispatch:\n", maxsplit=1
)[0]
push_paths = re.findall(r"^\s+- '([^']+)'\s*$", push_section, flags=re.MULTILINE)
pull_request_paths = re.findall(
    r"^\s+- '([^']+)'\s*$", pull_request_section, flags=re.MULTILINE
)
if push_paths != pull_request_paths:
    reject("push and pull_request path ownership must remain identical")

for path in trigger_paths:
    if path.startswith("docs/"):
        reject(f"documentation-only path still starts the long consumer gate: {path}")
    if path.startswith("rust/"):
        reject(
            "raw provider paths must advance the exact consumer reverse pin instead of "
            f"running a stale consumer revision: {path}"
        )

required_paths = (
    "studio/demo-consumer-source.json",
    "studio/libs/adapter-client/**",
    "studio/apps/studio-app/**",
    "studio/test/browser/**",
    "studio/test/entity-inspector-consumer-browser/**",
    "scripts/verify-studio-demo-integration.sh",
    "scripts/verify-studio-browser-integration.sh",
    "scripts/verify-studio-entity-inspector-integration.sh",
    "scripts/check-studio-demo-ci-policy.py",
    ".github/workflows/studio-demo-integration.yml",
)
for path in required_paths:
    if trigger_paths.count(path) != 2:
        reject(f"push and pull_request must both own trigger path {path}")

required_workflow_fragments = (
    "name: verify-studio-demo-browser",
    "name: verify-studio-demo-entity-inspector",
    'verify-studio-demo-integration.sh "$GITHUB_WORKSPACE/rusty-engine-demo" browser',
    'verify-studio-demo-integration.sh "$GITHUB_WORKSPACE/rusty-engine-demo" entity-inspector',
    "verify-studio-demo-integration:\n    needs: [demo_browser, entity_inspector]",
    "BROWSER_RESULT: ${{ needs.demo_browser.result }}",
    "ENTITY_INSPECTOR_RESULT: ${{ needs.entity_inspector.result }}",
    "workflow_dispatch:",
)
for fragment in required_workflow_fragments:
    if fragment not in workflow:
        reject(f"workflow is missing required parallel proof fragment: {fragment}")

if parent.count("pnpm --dir studio run build") != 1:
    reject("the parent gate must contain exactly one Studio build owner")
if parent.count(
    'cargo build --locked --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter'
) != 1:
    reject("the parent gate must contain exactly one locked adapter build owner")
for mode in ("all)", "browser)", "entity-inspector)"):
    if mode not in parent:
        reject(f"the parent gate is missing orchestration mode {mode[:-1]}")

for forbidden in ("pnpm --dir studio run build", "cargo build"):
    if forbidden in browser:
        reject(f"the browser helper must consume prepared artifacts, not run {forbidden}")
if "STUDIO_STATIC_ROOT" not in browser or "ADAPTER_BINARY" not in browser:
    reject("the browser helper must fail closed when prepared Studio or adapter artifacts are absent")
if "cargo build" in entity_inspector:
    reject("the entity-inspector helper must consume the parent-built adapter")

print(
    "Studio demo CI policy passed: exact triggers, two parallel browser owners, "
    "one aggregate gate, and no duplicate helper builds"
)
