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
            "render",
            "studio",
            "docs",
        )
    }
    concurrency = "group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true"
    for name, workflow in workflows.items():
        if concurrency not in workflow:
            fail(f"{name} does not cancel superseded runs per workflow/ref")

    require_paths(
        "verify",
        workflows["verify"],
        {
            "Cargo.toml",
            "Cargo.lock",
            "rust/**",
            "!rust/crates/renderer-webview-host/**",
            "scripts/verify.sh",
        },
        {"csharp/**", "fixtures/csharp-*/**", "render/**", "studio/**", "migration/**"},
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
            "scripts/test-csharp-binding-generator-lease-fixture.sh",
            "scripts/verify-csharp*.sh",
            "scripts/pack-csharp-sdk.sh",
            "scripts/test-csharp-sdk-package.sh",
        },
        {"Cargo.toml", "Cargo.lock", "rust/**", "render/**", "studio/**"},
    )

    require_paths(
        "render",
        workflows["render"],
        {
            "render/**",
            "rust/crates/render-host-contracts/**",
            "scripts/verify-render-artifacts.sh",
        },
        set(),
    )

    routing_cases = {
        "fixtures/csharp-nativeaot-trial/Product.cs": {"csharp"},
        "csharp/Rusty.Engine/Mechanics/Inventory.cs": {"csharp"},
        "rust/crates/csharp-engine-abi/src/lib.rs": {"csharp", "verify"},
        "rust/crates/csharp-engine-services/src/lib.rs": {"csharp", "verify"},
        "rust/crates/csharp-product-runtime/src/lib.rs": {"csharp", "verify"},
        "scripts/generate-csharp-native-bindings.sh": {"csharp"},
        "scripts/test-csharp-binding-generator-lease-fixture.sh": {"csharp"},
        "render/browser/application-host.browser.spec.ts": {"render"},
        "render/packages/renderer-three/src/backend.ts": {
            "render",
            "studio",
        },
        "rust/crates/renderer-webview-host/artifacts/renderer-webview.js": set(),
        "rust/crates/renderer-webview-host/src/lib.rs": set(),
        "rust/crates/entity-state/src/lib.rs": {"studio", "verify"},
        "docs/csharp-sdk.md": {"docs"},
        "studio/apps/studio-app/src/main.ts": {"studio"},
        ".github/workflows/render.yml": {"docs", "render"},
        "render/artifacts/application-host/index.js": {
            "render",
        },
        "render/artifacts/product-browser-host/product-browser-host.js": {
            "render",
        },
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
        "studio",
        workflows["studio"],
        {
            "studio/**",
            "render/packages/render-contracts/**",
            "render/packages/render-projection/**",
            "render/packages/renderer-host/**",
            "render/packages/renderer-three/**",
        },
        {"render/**", "render/browser/**", "render/artifacts/**", "fixtures/render/**"},
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
