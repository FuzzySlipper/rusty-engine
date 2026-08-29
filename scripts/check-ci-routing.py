#!/usr/bin/env python3
"""Check the repository-owned CI routing and single-pass renderer contract."""

from __future__ import annotations

import argparse
import json
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
            "render",
            "studio",
            "docs",
            "product-materializer",
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
            "!rust/crates/renderer-webview-host/artifacts/**",
            "scripts/verify.sh",
        },
        {"render/**", "studio/**", "rules/**"},
    )
    if "paths-ignore:" in workflows["verify"]:
        fail("verify must use explicit owner paths instead of a repository-wide paths-ignore")

    require_paths(
        "render",
        workflows["render"],
        {
            "render/**",
            "rust/crates/render-host-contracts/**",
            "rust/crates/renderer-webview-host/**",
            "rust/crates/developer-command/**",
            "scripts/verify-render-artifacts.sh",
            "scripts/verify-renderer-webview-host.sh",
        },
        set(),
    )

    routing_cases = {
        "fixtures/render/depth-splat-comparison-v1.json": {"render", "verify"},
        "render/browser/application-host.browser.spec.ts": {"render"},
        "render/packages/renderer-three/src/backend.ts": {
            "product-materializer",
            "render",
            "studio",
        },
        "render/artifacts/developer-command-client/index.js": {"render"},
        "render/packages/developer-command-client/src/index.ts": {
            "product-materializer",
            "render",
        },
        "rust/crates/developer-command/src/wire.rs": {"render", "verify"},
        "rust/crates/renderer-webview-host/artifacts/renderer-webview.js": {"render"},
        "rust/crates/renderer-webview-host/src/lib.rs": {"render", "verify"},
        "rust/crates/entity-state/src/lib.rs": {"studio", "verify"},
        "docs/csharp-sdk.md": {"docs"},
        "studio/apps/studio-app/src/main.ts": {"studio"},
        ".github/workflows/render.yml": {"docs", "render"},
        "rust/crates/product-materializer/src/lib.rs": {
            "product-materializer",
            "verify",
        },
        "fixtures/product-assembly/counter-kernel.rs": {
            "product-materializer",
            "verify",
        },
        "rules/packages/runtime-composition-authoring/src/index.ts": {
            "product-materializer",
        },
        "render/artifacts/application-host/index.js": {
            "product-materializer",
            "render",
        },
        "render/artifacts/product-browser-host/product-browser-host.js": {
            "product-materializer",
            "render",
        },
        "scripts/verify-product-materializer.sh": {"product-materializer"},
        ".github/workflows/product-materializer.yml": {
            "docs",
            "product-materializer",
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
    if "pnpm --dir render install --frozen-lockfile --ignore-scripts" not in workflows["render"]:
        fail("render CI dependency admission must not compile workspace packages")
    if "RUSTY_RENDER_DEPS_READY=1 ./scripts/verify-render.sh" not in workflows["render"]:
        fail("render CI must reuse its one admitted dependency installation")

    materializer = workflows["product-materializer"]
    for required in (
        "cargo build --locked -p rusty-engine",
        "pnpm --dir rules install --frozen-lockfile --ignore-scripts",
        "pnpm --dir rules run build",
        "pnpm --dir render install --frozen-lockfile --ignore-scripts",
        "pnpm --dir render run build:packages",
        "pnpm --dir render run bundle:application-host-artifact",
        "pnpm --dir render run bundle:product-browser-host-artifact",
        "./scripts/verify-product-materializer.sh",
    ):
        if required not in materializer:
            fail(f"product-materializer CI is missing {required}")
    if "./scripts/verify-render.sh" in materializer:
        fail("product-materializer CI must not repeat the full renderer browser gate")

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
    require_paths(
        "product-materializer",
        workflows["product-materializer"],
        {
            "Cargo.toml",
            "Cargo.lock",
            "rust/crates/product-materializer/**",
            "fixtures/product-assembly/**",
            "rules/packages/runtime-composition-authoring/**",
            "rules/tsconfig.base.json",
            "render/packages/application-host/**",
            "render/tsconfig.base.json",
            "render/artifacts/application-host/**",
            "scripts/verify-product-materializer.sh",
            ".github/workflows/product-materializer.yml",
        },
        {"rust/**", "rules/**", "render/**", "fixtures/**"},
    )

    for name in ("verify", "render", "product-materializer"):
        workflow = workflows[name]
        if "uses: Swatinem/rust-cache@v2" not in workflow or "shared-key: engine-ci" not in workflow:
            fail(f"{name} does not participate in the bounded shared Rust cache")

    aggregate = read(root, "scripts/verify-render.sh")
    if 'install --frozen-lockfile --ignore-scripts' not in aggregate:
        fail("local renderer dependency admission must not compile workspace packages")
    for required in (
        '"$REPO_ROOT/scripts/verify-render-artifacts.sh"',
        '"$REPO_ROOT/scripts/verify-renderer-webview-host.sh" --artifacts-ready',
        'run typecheck:browser',
        'run test:compiled',
        'run test:browser',
    ):
        if required not in aggregate:
            fail(f"aggregate renderer gate is missing {required}")
    for duplicate in ("verify-application-host-artifact.sh", "run verify"):
        if duplicate in aggregate:
            fail(f"aggregate renderer gate reintroduced duplicate path {duplicate}")

    artifact_gate = read(root, "scripts/verify-render-artifacts.sh")
    if artifact_gate.count('run build\n') != 1:
        fail("combined artifact gate must perform exactly one renderer build")
    for artifact in ("application-host", "developer-command-client", "renderer-webview.js"):
        if artifact not in artifact_gate:
            fail(f"combined artifact gate does not check {artifact}")

    render_package = json.loads(read(root, "render/package.json"))
    scripts = render_package.get("scripts", {})
    if "test:compiled" not in scripts or "typecheck:browser" not in scripts:
        fail("renderer root does not expose compiled-test and browser-typecheck phases")
    for package in (
        "application-host",
        "developer-command-client",
        "render-contracts",
        "render-projection",
        "renderer-host",
        "renderer-three",
    ):
        data = json.loads(read(root, f"render/packages/{package}/package.json"))
        if "test:compiled" not in data.get("scripts", {}):
            fail(f"renderer package {package} cannot reuse compiled output")

    print("ci routing and single-pass renderer contract passed")


if __name__ == "__main__":
    main()
