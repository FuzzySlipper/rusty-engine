#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-ci-routing.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT

copy_workflows() {
  rm -rf "$PROBE_ROOT/.github"
  mkdir -p "$PROBE_ROOT/.github"
  cp -a "$REPO_ROOT/.github/workflows" "$PROBE_ROOT/.github/workflows"
}

rewrite_event_path() {
  local workflow_file="$1"
  local event="$2"
  local path_pattern="$3"
  local remove_path="$4"
  python3 - "$workflow_file" "$event" "$path_pattern" "$remove_path" <<'PY'
from pathlib import Path
import sys
import yaml

path = Path(sys.argv[1])
event, path_pattern, remove_path = sys.argv[2:]
document = yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
trigger = document["on"][event]
if trigger == "":
    trigger = {}
    document["on"][event] = trigger
if remove_path == "true":
    paths = trigger["paths"]
    updated = [pattern for pattern in paths if pattern != path_pattern]
    if len(updated) == len(paths):
        raise SystemExit(f"path filter was not present: {event} {path_pattern}")
    trigger["paths"] = updated
for name, value in document["on"].items():
    if value == "":
        document["on"][name] = {}
path.write_text(yaml.safe_dump(document, sort_keys=False), encoding="utf-8")
PY
}

expect_rejection() {
  local label="$1"
  if python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null 2>&1; then
    echo "ci routing checker accepted negative probe: $label" >&2
    exit 1
  fi
}

python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$REPO_ROOT" >/dev/null

# PyYAML's normal formatting changes indentation, quotes, and block style. It
# must remain accepted before the deliberately asymmetric route loss below.
copy_workflows
rewrite_event_path "$PROBE_ROOT/.github/workflows/browser.yml" push "render/**" false
if ! python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null; then
  echo "ci routing checker rejected reformatted workflow YAML" >&2
  exit 1
fi

# The PR route remains present. Removing only push coverage must still fail.
rewrite_event_path "$PROBE_ROOT/.github/workflows/browser.yml" push "render/**" true
expect_rejection "missing browser push owner with pull-request owner retained"

# Every event must own the script that supplies its Rust architecture checks.
copy_workflows
rewrite_event_path \
  "$PROBE_ROOT/.github/workflows/verify.yml" pull_request \
  "scripts/test_architecture_checks.py" true
expect_rejection "missing pull-request route for the architecture test helper"

echo "ci routing checker event-specific negative probes passed"
