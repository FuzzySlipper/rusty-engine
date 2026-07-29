#!/usr/bin/env python3
"""Report non-blocking drift between Cargo workspace owners and curated code maps."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import subprocess
from typing import Any
from urllib.parse import unquote, urlsplit


REPO_ROOT = Path(__file__).resolve().parents[1]
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


@dataclass(frozen=True)
class Advisory:
    message: str
    path: Path | None = None
    line: int | None = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--metadata",
        type=Path,
        help="read Cargo metadata JSON from this file instead of invoking Cargo",
    )
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="repository root")
    return parser.parse_args()


def load_metadata(root: Path, metadata_path: Path | None = None) -> dict[str, Any]:
    if metadata_path is not None:
        with metadata_path.open(encoding="utf-8") as source:
            return json.load(source)
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def workspace_package_roots(metadata: dict[str, Any]) -> dict[str, Path]:
    workspace_ids = set(metadata["workspace_members"])
    roots = {
        package["name"]: Path(package["manifest_path"]).resolve().parent
        for package in metadata["packages"]
        if package["id"] in workspace_ids
    }
    if len(roots) != len(workspace_ids):
        raise ValueError("workspace package names are not unique or metadata is incomplete")
    return roots


def primary_path_links(page: Path) -> list[tuple[int, str]]:
    links: list[tuple[int, str]] = []
    in_primary_paths = False
    for line_number, line in enumerate(page.read_text(encoding="utf-8").splitlines(), start=1):
        if line == "## Primary paths":
            in_primary_paths = True
            continue
        if in_primary_paths and line.startswith("## "):
            break
        if in_primary_paths:
            links.extend((line_number, match.group(1)) for match in MARKDOWN_LINK.finditer(line))
    return links


def resolve_local_link(page: Path, raw_target: str) -> Path | None:
    target = raw_target.strip().strip("<>").split(maxsplit=1)[0]
    parsed = urlsplit(target)
    if parsed.scheme or parsed.netloc or not parsed.path:
        return None
    return (page.parent / unquote(parsed.path)).resolve()


def find_advisories(root: Path, metadata: dict[str, Any]) -> list[Advisory]:
    package_roots = workspace_package_roots(metadata)
    crate_parent = (root / "rust" / "crates").resolve()
    assignments = {name: set() for name in package_roots}
    advisories: list[Advisory] = []

    map_directory = root / "docs" / "code-map"
    pages = sorted(map_directory.glob("*.md"))
    if not pages:
        return [Advisory("no curated pages were found under docs/code-map")]

    for page in pages:
        for line_number, raw_target in primary_path_links(page):
            resolved = resolve_local_link(page, raw_target)
            if resolved is None:
                continue
            relative_page = page.relative_to(root)
            if not resolved.exists():
                advisories.append(
                    Advisory(
                        f"unresolved primary-path reference: {raw_target}",
                        relative_page,
                        line_number,
                    )
                )

            assigned = False
            for name, package_root in package_roots.items():
                if is_within(resolved, package_root):
                    assignments[name].add(relative_page.as_posix())
                    assigned = True
            if is_within(resolved, crate_parent) and not assigned:
                advisories.append(
                    Advisory(
                        f"stale Cargo crate path is not a current workspace member: {raw_target}",
                        relative_page,
                        line_number,
                    )
                )

    for name in sorted(assignments):
        if not assignments[name]:
            manifest = package_roots[name] / "Cargo.toml"
            try:
                manifest_display = manifest.relative_to(root).as_posix()
            except ValueError:
                manifest_display = str(manifest)
            advisories.append(
                Advisory(
                    f"missing owner-map assignment for Cargo package {name} ({manifest_display})"
                )
            )
    return sorted(
        advisories,
        key=lambda advisory: (
            advisory.path.as_posix() if advisory.path is not None else "",
            advisory.line or 0,
            advisory.message,
        ),
    )


def is_within(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def annotation_escape(value: str) -> str:
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def report(package_count: int, advisories: list[Advisory]) -> None:
    if advisories:
        print(
            f"code-map freshness advisory: {len(advisories)} issue(s); "
            "verification remains green"
        )
        for advisory in advisories:
            location = ""
            if advisory.path is not None:
                location = advisory.path.as_posix()
                if advisory.line is not None:
                    location += f":{advisory.line}"
                location += ": "
            print(f"- {location}{advisory.message}")
            if os.environ.get("GITHUB_ACTIONS") == "true":
                properties = "title=Rusty Engine code-map freshness"
                if advisory.path is not None:
                    properties = f"file={advisory.path.as_posix()}," + properties
                if advisory.line is not None:
                    properties = f"line={advisory.line}," + properties
                print(f"::warning {properties}::{annotation_escape(advisory.message)}")
    else:
        print(
            f"code-map freshness advisory passed: {package_count} workspace packages "
            "have primary-path owners"
        )

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with Path(summary_path).open("a", encoding="utf-8") as summary:
            summary.write("### Rusty Engine code-map freshness\n\n")
            if advisories:
                summary.write(
                    f"Found {len(advisories)} non-blocking issue(s) across "
                    f"{package_count} Cargo workspace packages.\n\n"
                )
                for advisory in advisories:
                    summary.write(f"- {advisory.message}\n")
            else:
                summary.write(
                    f"All {package_count} Cargo workspace packages have a curated "
                    "primary-path owner, and all primary paths resolve.\n"
                )
            summary.write("\n")


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    try:
        metadata = load_metadata(root, args.metadata)
        package_roots = workspace_package_roots(metadata)
        advisories = find_advisories(root, metadata)
    except (
        OSError,
        subprocess.CalledProcessError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        advisories = [Advisory(f"freshness checker could not inspect the repository: {error}")]
        package_roots = {}
    report(len(package_roots), advisories)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
