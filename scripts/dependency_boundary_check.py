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
        "gameplay-mechanics",
        "state-machine",
        "voxel-annotation",
        "voxel-asset",
        "voxel-convert",
        "voxel-object-runtime",
    }
)
GAMEPLAY_AUTHORITY = frozenset(
    {"gameplay-mechanics", "gameplay-resolution", "gameplay-rules"}
)
GAMEPLAY_RESOLUTION_FORBIDDEN = frozenset(
    {"entity-state", "gameplay-mechanics", "gameplay-rules"}
)
RENDER_HOST_BACKEND_PACKAGES = frozenset({"renderer-host", "renderer-three"})
RENDER_MODEL_FORBIDDEN = (
    ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS
    | GAMEPLAY_AUTHORITY
    | RENDER_HOST_BACKEND_PACKAGES
    | {"engine-inspector", "render-presentation", "render-projection"}
)
RENDER_PRESENTATION_FORBIDDEN = (
    ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS
    | GAMEPLAY_AUTHORITY
    | RENDER_HOST_BACKEND_PACKAGES
    | {"engine-inspector", "render-projection"}
)


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

        if source_name == "gameplay-resolution":
            for target in reachable:
                if names[target] in GAMEPLAY_RESOLUTION_FORBIDDEN:
                    violations.add(
                        "gameplay-resolution reaches a downstream-selected gameplay owner: "
                        + render_path(source, target, parents, names)
                    )

        if source_name not in {"engine-inspector", "rusty-engine"}:
            for target in reachable:
                if names[target] == "engine-inspector":
                    violations.add(
                        "ordinary workspace package reaches the engine-inspector leaf: "
                        + render_path(source, target, parents, names)
                    )

        if source_name in ENTITY_SPATIAL_CONTENT_ASSET_VOXEL_OWNERS:
            for target in reachable:
                if names[target] == "render-projection":
                    violations.add(
                        "authoritative owner reverse-depends on render-projection: "
                        + render_path(source, target, parents, names)
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
        elif source_name == "render-projection":
            for target in reachable:
                if names[target] in GAMEPLAY_AUTHORITY:
                    violations.add(
                        "render-projection reaches gameplay authority: "
                        + render_path(source, target, parents, names)
                    )

    return sorted(violations)


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
