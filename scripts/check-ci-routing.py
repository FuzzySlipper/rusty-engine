#!/usr/bin/env python3
"""Check a few meaningful repository paths against each CI event's owners."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    print(
        "CI routing check requires PyYAML; install scripts/requirements-ci.txt",
        file=sys.stderr,
    )
    raise SystemExit(2)


WORKFLOWS = ("verify", "csharp", "browser", "docs", "pair", "shaders")


def fail(message: str) -> None:
    print(f"ci routing check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def read(root: Path, relative: str) -> str:
    path = root / relative
    if not path.is_file():
        fail(f"missing {relative}")
    return path.read_text(encoding="utf-8")


def load_workflow(root: Path, name: str) -> dict[str, list[str]]:
    source = read(root, f".github/workflows/{name}.yml")
    try:
        document = yaml.load(source, Loader=yaml.BaseLoader)
    except yaml.YAMLError as error:
        fail(f"{name} is not valid workflow YAML: {error}")
    if not isinstance(document, dict) or not isinstance(document.get("on"), dict):
        fail(f"{name} has no workflow event map")

    # BaseLoader keeps the Actions key `on` as a string instead of YAML 1.1's
    # boolean. Other YAML formatting is handled by the parser.
    filters: dict[str, list[str]] = {}
    for event in ("push", "pull_request"):
        if event not in document["on"]:
            filters[event] = []
            continue
        trigger = document["on"][event]
        if trigger is None or trigger == "" or trigger == {}:
            filters[event] = ["**"]
            continue
        if not isinstance(trigger, dict):
            fail(f"{name} has an invalid {event} trigger")
        paths = trigger.get("paths", [])
        if not paths:
            filters[event] = ["**"]
            continue
        if not isinstance(paths, list) or not all(isinstance(path, str) for path in paths):
            fail(f"{name} has invalid {event} path filters")
        filters[event] = paths
    return filters


def path_matches(pattern: str, path: str) -> bool:
    expression = re.escape(pattern).replace(r"\*\*", ".*").replace(r"\*", "[^/]*")
    return re.fullmatch(expression, path) is not None


def workflow_routes(patterns: list[str], path: str) -> bool:
    routed = False
    for pattern in patterns:
        excluded = pattern.startswith("!")
        candidate = pattern[1:] if excluded else pattern
        if path_matches(candidate, path):
            routed = not excluded
    return routed


def require_routes(
    workflows: dict[str, dict[str, list[str]]],
    event: str,
    path: str,
    expected: set[str],
) -> None:
    actual = {
        name
        for name, events in workflows.items()
        if workflow_routes(events.get(event, []), path)
    }
    if actual != expected:
        fail(
            f"{event} path {path} routes to {sorted(actual)}, "
            f"expected {sorted(expected)}"
        )


def require_both_events(
    workflows: dict[str, dict[str, list[str]]], paths: list[str], expected: set[str]
) -> None:
    for path in paths:
        for event in ("push", "pull_request"):
            require_routes(workflows, event, path, expected)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    root = args.root.resolve()

    event_paths = {name: load_workflow(root, name) for name in WORKFLOWS}

    # Keep these as representative ownership checks: one source from each
    # maintained lane and each helper whose edits change what that lane checks.
    require_both_events(
        event_paths,
        [
            "scripts/test-csharp-binding-generator-results-fixture.sh",
            "scripts/test-csharp-sdk-package.sh",
            "scripts/verify-csharp.sh",
        ],
        {"csharp"},
    )
    require_both_events(
        event_paths,
        [
            "scripts/audit-standalone.sh",
            "scripts/dependency_boundary_check.py",
            "scripts/test_architecture_checks.py",
            "scripts/verify.sh",
        ],
        {"verify"},
    )
    require_both_events(
        event_paths,
        [
            "scripts/performance-results.test.mjs",
            "scripts/capture-playtest-warning-delta.test.mjs",
        ],
        {"browser"},
    )
    require_both_events(
        event_paths,
        ["docs/verification.md", "scripts/requirements-ci.txt"],
        {"docs"},
    )
    require_both_events(event_paths, [".github/workflows/browser.yml"], {"browser", "docs"})
    require_both_events(event_paths, [".github/workflows/csharp.yml"], {"csharp", "docs"})
    require_both_events(event_paths, [".github/workflows/shaders.yml"], {"shaders", "docs"})
    require_both_events(event_paths, [".github/workflows/verify.yml"], {"verify", "docs"})
    require_both_events(event_paths, ["rust-toolchain.toml"], {"shaders"})

    event_cases = {
        "rust/crates/entity-state/src/lib.rs": ({"verify", "pair"}, {"verify"}),
        "rust/crates/csharp-engine-abi/src/lib.rs": (
            {"verify", "csharp", "pair"}, {"verify", "csharp"}
        ),
        "csharp/Rusty.Engine/Mechanics/Inventory.cs": ({"csharp", "pair"}, {"csharp"}),
        "fixtures/csharp-nativeaot-trial/Product.cs": ({"csharp", "pair"}, {"csharp"}),
        "scripts/generate-csharp-native-bindings.sh": ({"csharp", "pair"}, {"csharp"}),
        "scripts/pack-csharp-sdk.sh": ({"csharp", "pair"}, {"csharp"}),
        "scripts/fixtures/EngineTestHostChecks.cs": ({"csharp", "pair"}, {"csharp"}),
        "render/packages/application-host/src/index.ts": ({"browser", "pair"}, {"browser"}),
        "render/packages/application-host/src/generated/contracts.ts": (
            {"verify", "browser", "pair"}, {"verify", "browser"}
        ),
        "rust/crates/render-shaders/src/lib.rs": (
            {"verify", "pair", "shaders"}, {"verify", "shaders"}
        ),
        "rust/crates/render-wgpu/src/apply.rs": ({"verify", "pair"}, {"verify"}),
        "Cargo.toml": ({"verify", "pair", "shaders"}, {"verify", "shaders"}),
        "Cargo.lock": ({"verify", "pair", "shaders"}, {"verify", "shaders"}),
    }
    for path, (push_owners, pr_owners) in event_cases.items():
        require_routes(event_paths, "push", path, push_owners)
        require_routes(event_paths, "pull_request", path, pr_owners)

    for path in (
        "scripts/pair-platform.sh",
        "scripts/third-party-notices.py",
        "scripts/inventory-validation.py",
        "scripts/build-runtime-pack.sh",
        "scripts/test-csharp-release-pair.sh",
    ):
        require_routes(event_paths, "push", path, {"pair"})
    require_routes(event_paths, "push", ".github/workflows/pair.yml", {"pair", "docs"})

    print("CI owner routing passed")


if __name__ == "__main__":
    main()
