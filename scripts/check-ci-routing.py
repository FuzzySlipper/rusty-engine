#!/usr/bin/env python3
"""Check the repository-owned CI routing for active verification lanes."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


def fail(message: str) -> None:
    print(f"ci routing check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def read(root: Path, relative: str) -> str:
    path = root / relative
    if not path.is_file():
        fail(f"missing {relative}")
    return path.read_text(encoding="utf-8")


def quoted_paths(workflow: str) -> set[str]:
    return set(re.findall(r"^\s{6}- '([^']+)'\s*$", workflow, flags=re.MULTILINE))


def ordered_paths(workflow: str) -> list[str]:
    return re.findall(r"^\s{6}- '([^']+)'\s*$", workflow, flags=re.MULTILINE)


def path_matches(pattern: str, path: str) -> bool:
    expression = re.escape(pattern).replace(r"\*\*", ".*").replace(r"\*", "[^/]*")
    return re.fullmatch(expression, path) is not None


def workflow_routes(workflow: str, path: str) -> bool:
    routed = False
    for pattern in ordered_paths(workflow):
        excluded = pattern.startswith("!")
        candidate = pattern[1:] if excluded else pattern
        if path_matches(candidate, path):
            routed = not excluded
    return routed


def require_paths(name: str, workflow: str, required: set[str], forbidden: set[str]) -> None:
    paths = quoted_paths(workflow)
    missing = sorted(required - paths)
    unexpected = sorted(forbidden & paths)
    if missing:
        fail(f"{name} is missing owned paths: {', '.join(missing)}")
    if unexpected:
        fail(f"{name} contains over-broad paths: {', '.join(unexpected)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    root = args.root.resolve()

    workflows = {
        name: read(root, f".github/workflows/{name}.yml")
        for name in (
            "verify",
            "csharp",
            "browser",
            "docs",
        )
    }
    # A pull request's newer run cancels its older one; every pushed commit
    # keeps its own run, because each can be a review gate.
    concurrency = (
        "group: ${{ github.workflow }}-${{ github.event_name == 'push' && github.sha || github.ref }}"
        "\n  cancel-in-progress: true"
    )
    for name, workflow in workflows.items():
        if concurrency not in workflow:
            fail(f"{name} does not cancel superseded pull request runs while keeping each pushed commit's run")

    require_paths(
        "verify",
        workflows["verify"],
        {
            "Cargo.toml",
            "Cargo.lock",
            "rust/**",
            # The contracts Rust emits; a stale one fails a Rust test.
            "render/packages/*/src/generated/**",
            "scripts/verify.sh",
        },
        {"csharp/**", "fixtures/csharp-*/**", "render/**", "migration/**"},
    )
    if "paths-ignore:" in workflows["verify"]:
        fail("verify must use explicit owner paths instead of a repository-wide paths-ignore")

    require_paths(
        "csharp",
        workflows["csharp"],
        {
            "csharp/**",
            "fixtures/csharp-*/**",
            "rust/crates/csharp-engine-abi/**",
            "rust/crates/csharp-engine-services/**",
            "rust/crates/csharp-product-runtime/**",
            ".config/dotnet-tools.json",
            "scripts/generate-csharp-native-bindings.sh",
            "scripts/test-csharp-binding-generator-results-fixture.sh",
            "scripts/verify-csharp*.sh",
            "scripts/pack-csharp-sdk.sh",
            "scripts/test-csharp-sdk-package.sh",
        },
        {"Cargo.toml", "Cargo.lock", "rust/**", "render/**"},
    )

    # The browser workflow is the Node lane: it also owns the Node script
    # tooling tests.
    require_paths(
        "browser",
        workflows["browser"],
        {
            "render/**",
            "scripts/capture-playtest-warning-delta*.mjs",
            "scripts/performance-results*.mjs",
        },
        {"rust/**", "csharp/**", "scripts/**"},
    )

    routing_cases = {
        "fixtures/csharp-nativeaot-trial/Product.cs": {"csharp"},
        "csharp/Rusty.Engine/Mechanics/Inventory.cs": {"csharp"},
        "rust/crates/csharp-engine-abi/src/lib.rs": {"csharp", "verify"},
        "rust/crates/csharp-engine-services/src/lib.rs": {"csharp", "verify"},
        "rust/crates/csharp-product-runtime/src/lib.rs": {"csharp", "verify"},
        "scripts/generate-csharp-native-bindings.sh": {"csharp"},
        "scripts/test-csharp-binding-generator-results-fixture.sh": {"csharp"},
        "render/packages/product-browser-host/src/index.ts": {"browser"},
        "render/packages/live-debug-panel/src/browser-mount.ts": {"browser"},
        "render/packages/application-host/src/generated/contracts.ts": {"browser", "verify"},
        "scripts/performance-results.mjs": {"browser"},
        "scripts/performance-results.test.mjs": {"browser"},
        "scripts/capture-playtest-warning-delta.mjs": {"browser"},
        "scripts/capture-playtest-warning-delta.test.mjs": {"browser"},
        "rust/crates/entity-state/src/lib.rs": {"verify"},
        "docs/csharp-sdk.md": {"docs"},
        ".github/workflows/browser.yml": {"docs", "browser"},
    }
    for path, expected in routing_cases.items():
        actual = {
            name for name, workflow in workflows.items()
            if workflow_routes(workflow, path)
        }
        if actual != expected:
            fail(
                f"{path} routes to {sorted(actual)}, expected {sorted(expected)}"
            )
    require_paths(
        "docs",
        workflows["docs"],
        {"README.md", "AGENTS.md", "docs/**", ".github/workflows/**"},
        set(),
    )
    print("CI owner routing passed")


if __name__ == "__main__":
    main()
