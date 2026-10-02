#!/usr/bin/env python3
"""Check the small set of hard Rust workspace dependency boundaries."""

from __future__ import annotations

import argparse
from collections import deque
import json
from pathlib import Path
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]

ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS = frozenset(
    {
        "asset-catalog",
        "asset-import",
        "authored-scene",
        "content-store",
        "engine-spatial",
        "entity-state",
        "environment-authoring",
        "voxel-annotation",
        "voxel-asset",
        "voxel-convert",
        "voxel-object-runtime",
    }
)
# The renderer and its hosts consume the retained model, never the reverse.
RENDER_BACKEND_PACKAGES = frozenset({"render-wgpu", "render-stream", "desktop-shell"})
RENDER_MODEL_FORBIDDEN = (
    ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS
    | RENDER_BACKEND_PACKAGES
    | {"render-presentation", "render-projection"}
)
RENDER_PRESENTATION_FORBIDDEN = (
    ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS
    | RENDER_BACKEND_PACKAGES
    | {"render-projection"}
)
# The only workspace crates the renderer may depend on, so the runtime, its
# host and its publications never enter it.
RENDER_WGPU_WORKSPACE_DEPENDENCIES = frozenset(
    {
        "render-host-contracts",
        "render-model",
        "render-presentation",
        "render-shaders",
        "render-video",
    }
)
# External crates that only the named workspace crates may depend on, so a
# device or backend library stays behind its owner's API.
EXTERNAL_DEPENDENCY_OWNERS = {
    "cpal": frozenset({"render-audio"}),
    "jpeg-encoder": frozenset({"render-stream"}),
    "kira": frozenset({"render-audio"}),
    "opus-decoder": frozenset({"render-audio"}),
    "symphonia": frozenset({"render-audio"}),
    "fontdue": frozenset({"render-wgpu"}),
    "glam": frozenset({"render-wgpu"}),
    # Shader composition and checking (render-shaders); wgpu's own naga.
    "naga": frozenset({"render-shaders", "render-wgpu"}),
    "naga_oil": frozenset({"render-shaders"}),
    "wgpu": frozenset({"render-wgpu"}),
    "wgpu-core": frozenset({"render-wgpu"}),
    "wgpu-hal": frozenset({"render-wgpu"}),
    "wgpu-types": frozenset({"render-wgpu"}),
    "wuff": frozenset({"render-wgpu"}),
    # The desktop shell's UI overlay (Chromium off-screen rendering).
    "cef": frozenset({"render-wgpu"}),
    "welding": frozenset({"render-wgpu"}),
    "winit": frozenset({"desktop-shell"}),
    # Video clips: WebM demux and VP9 decode (#8791).
    "matroska-demuxer": frozenset({"render-video"}),
    "rusty_vp9": frozenset({"render-video"}),
    # TypeScript declarations are generated only for the host's wire; the
    # render crates' types never reach the page.
    "ts-rs": frozenset(
        {"product-host", "runtime-diagnostics", "runtime-input", "runtime-ui"}
    ),
}

# External crates a workspace crate may not depend on, because the work that
# needs them has its own owner.
FORBIDDEN_EXTERNAL_DEPENDENCIES = {
    # Writing glTF documents belongs to render-export, not GPU realization.
    "render-wgpu": frozenset({"serde_json"}),
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--metadata",
        type=Path,
        help="read Cargo metadata JSON from this file instead of invoking Cargo",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=REPO_ROOT,
        help="repository root used when invoking cargo metadata",
    )
    return parser.parse_args()


def load_metadata(root: Path, metadata_path: Path | None = None) -> dict[str, Any]:
    if metadata_path is not None:
        with metadata_path.open(encoding="utf-8") as source:
            return json.load(source)
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def workspace_graph(
    metadata: dict[str, Any],
) -> tuple[dict[str, str], dict[str, set[str]]]:
    workspace_ids = set(metadata["workspace_members"])
    names = {
        package["id"]: package["name"]
        for package in metadata["packages"]
        if package["id"] in workspace_ids
    }
    missing_packages = workspace_ids.difference(names)
    if missing_packages:
        raise ValueError(f"workspace package identities are missing: {sorted(missing_packages)}")

    graph = {package_id: set() for package_id in workspace_ids}
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        raise ValueError("cargo metadata did not include a resolved dependency graph")
    for node in resolve.get("nodes", []):
        source = node.get("id")
        if source not in workspace_ids:
            continue
        for dependency in node.get("deps", []):
            target = dependency.get("pkg")
            if target not in workspace_ids:
                continue
            dependency_kinds = dependency.get("dep_kinds", [])
            if not dependency_kinds or any(
                kind.get("kind") in (None, "build") for kind in dependency_kinds
            ):
                graph[source].add(target)
    return names, graph


def shortest_paths(start: str, graph: dict[str, set[str]], names: dict[str, str]) -> dict[str, str]:
    parents: dict[str, str] = {}
    queue = deque([start])
    visited = {start}
    while queue:
        current = queue.popleft()
        for target in sorted(
            graph[current], key=lambda package_id: (names[package_id], package_id)
        ):
            if target in visited:
                continue
            visited.add(target)
            parents[target] = current
            queue.append(target)
    return parents


def render_path(start: str, target: str, parents: dict[str, str], names: dict[str, str]) -> str:
    path = [target]
    while path[-1] != start:
        path.append(parents[path[-1]])
    path.reverse()
    return " -> ".join(names[package_id] for package_id in path)


def find_violations(metadata: dict[str, Any]) -> list[str]:
    names, graph = workspace_graph(metadata)
    violations: set[str] = set()
    for source in sorted(graph, key=lambda package_id: (names[package_id], package_id)):
        source_name = names[source]
        parents = shortest_paths(source, graph, names)
        reachable = sorted(parents, key=lambda package_id: (names[package_id], package_id))

        if source_name.startswith("core-"):
            for target in reachable:
                if not names[target].startswith("core-"):
                    violations.add(
                        "core foundation reaches a non-core workspace owner: "
                        + render_path(source, target, parents, names)
                    )

        if source_name.startswith("svc-"):
            for target in reachable:
                target_name = names[target]
                if not target_name.startswith(("core-", "svc-")):
                    violations.add(
                        "service mechanism reaches an upper-layer workspace owner: "
                        + render_path(source, target, parents, names)
                    )

        if source_name in ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS:
            for target in reachable:
                if names[target] == "render-projection":
                    violations.add(
                        "authoritative owner reverse-depends on render-projection: "
                        + render_path(source, target, parents, names)
                    )

        if source_name == "render-wgpu":
            for target in sorted(graph[source], key=lambda package_id: names[package_id]):
                if names[target] not in RENDER_WGPU_WORKSPACE_DEPENDENCIES:
                    violations.add(
                        f"render-wgpu depends on {names[target]}, outside the renderer's "
                        "workspace dependencies"
                    )

        if source_name == "render-model":
            add_forbidden_render_paths(
                source,
                reachable,
                parents,
                names,
                RENDER_MODEL_FORBIDDEN,
                violations,
            )
        elif source_name == "render-presentation":
            add_forbidden_render_paths(
                source,
                reachable,
                parents,
                names,
                RENDER_PRESENTATION_FORBIDDEN,
                violations,
            )

    add_external_owner_violations(metadata, violations)
    return sorted(violations)


def add_external_owner_violations(metadata: dict[str, Any], violations: set[str]) -> None:
    workspace_ids = set(metadata["workspace_members"])
    for package in metadata["packages"]:
        if package["id"] not in workspace_ids:
            continue
        for dependency in package.get("dependencies", []):
            if dependency.get("kind") == "dev":
                continue
            if dependency["name"] in FORBIDDEN_EXTERNAL_DEPENDENCIES.get(
                package["name"], frozenset()
            ):
                violations.add(f"{package['name']} may not depend on {dependency['name']}")
            owners = EXTERNAL_DEPENDENCY_OWNERS.get(dependency["name"])
            if owners is not None and package["name"] not in owners:
                violations.add(
                    f"{package['name']} depends on {dependency['name']}, "
                    f"which only {', '.join(sorted(owners))} may depend on"
                )


def add_forbidden_render_paths(
    source: str,
    reachable: list[str],
    parents: dict[str, str],
    names: dict[str, str],
    forbidden_packages: frozenset[str],
    violations: set[str],
) -> None:
    for target in reachable:
        target_name = names[target]
        if target_name not in forbidden_packages:
            continue
        violations.add(
            "renderer-neutral model reaches authority, projection, or host/backend code: "
            + render_path(source, target, parents, names)
        )


def main() -> int:
    args = parse_args()
    try:
        metadata = load_metadata(args.root.resolve(), args.metadata)
        names, graph = workspace_graph(metadata)
        violations = find_violations(metadata)
    except (
        OSError,
        subprocess.CalledProcessError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"dependency boundary checker failed: {error}", file=sys.stderr)
        return 2

    if violations:
        print("dependency boundary violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1

    edge_count = sum(len(targets) for targets in graph.values())
    print(
        f"dependency boundary check passed: {len(names)} workspace packages, "
        f"{edge_count} normal/build edges"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
