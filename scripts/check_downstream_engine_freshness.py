#!/usr/bin/env python3
"""Fail when a rolling downstream Cargo.lock is behind rusty-engine main."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib


DEFAULT_REPOSITORY = "https://github.com/FuzzySlipper/rusty-engine"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lockfile", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--branch", default="main")
    parser.add_argument("--expected-sha")
    return parser.parse_args()


def validate_manifest(manifest: Path, repository: str, branch: str) -> None:
    with manifest.open("rb") as source:
        cargo = tomllib.load(source)
    tables = [
        cargo.get("dependencies", {}),
        cargo.get("dev-dependencies", {}),
        cargo.get("build-dependencies", {}),
    ]
    workspace = cargo.get("workspace", {})
    if isinstance(workspace, dict):
        tables.append(workspace.get("dependencies", {}))
    targets = cargo.get("target", {})
    if isinstance(targets, dict):
        for target in targets.values():
            if isinstance(target, dict):
                tables.extend(
                    target.get(name, {})
                    for name in ("dependencies", "dev-dependencies", "build-dependencies")
                )

    canonical: list[tuple[str, object]] = []
    for table in tables:
        if not isinstance(table, dict):
            continue
        for alias, specification in table.items():
            if isinstance(specification, dict) and specification.get("git") == repository:
                canonical.append((alias, specification))
    if len(canonical) != 1:
        raise ValueError(
            "Cargo.toml must declare exactly one dependency from the canonical Engine repository"
        )
    alias, specification = canonical[0]
    package = specification.get("package", alias)
    if package != "rusty-engine":
        raise ValueError("the canonical Engine dependency must be the complete rusty-engine facade")
    if specification.get("branch") != branch or "rev" in specification or "tag" in specification:
        raise ValueError(f"rusty-engine must follow rolling branch {branch!r} during development")


def resolved_revision(lockfile: Path, repository: str, branch: str) -> str:
    with lockfile.open("rb") as source:
        lock = tomllib.load(source)
    packages = [package for package in lock.get("package", []) if package.get("name") == "rusty-engine"]
    if len(packages) != 1:
        raise ValueError("Cargo.lock must contain exactly one rusty-engine package")
    source = packages[0].get("source")
    expected_prefix = f"git+{repository}?branch={branch}#"
    if not isinstance(source, str) or not source.startswith(expected_prefix):
        raise ValueError(
            f"rusty-engine must be a rolling branch dependency with source {expected_prefix}<sha>"
        )
    revision = source.removeprefix(expected_prefix)
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise ValueError("rusty-engine lock source does not end in an exact 40-character SHA")
    return revision


def branch_head(repository: str, branch: str) -> str:
    completed = subprocess.run(
        ["git", "ls-remote", repository, f"refs/heads/{branch}"],
        check=True,
        capture_output=True,
        text=True,
    )
    fields = completed.stdout.split()
    if len(fields) != 2 or re.fullmatch(r"[0-9a-f]{40}", fields[0]) is None:
        raise ValueError(f"could not resolve {repository} branch {branch}")
    return fields[0]


def check_freshness(
    lockfile: Path,
    repository: str,
    branch: str,
    expected_sha: str | None,
) -> tuple[str, str]:
    resolved = resolved_revision(lockfile, repository, branch)
    expected = expected_sha or branch_head(repository, branch)
    if re.fullmatch(r"[0-9a-f]{40}", expected) is None:
        raise ValueError("expected revision must be a 40-character lowercase SHA")
    if resolved != expected:
        raise RuntimeError(
            f"downstream Engine lock is stale: resolved {resolved}, current {branch} is {expected}"
        )
    return resolved, expected


def main() -> int:
    args = parse_args()
    try:
        if args.manifest is not None:
            validate_manifest(args.manifest.resolve(), args.repository, args.branch)
        resolved, _ = check_freshness(
            args.lockfile.resolve(), args.repository, args.branch, args.expected_sha
        )
    except (OSError, subprocess.CalledProcessError, ValueError, RuntimeError) as error:
        print(f"Engine freshness check failed: {error}", file=sys.stderr)
        return 1
    print(f"Engine freshness check passed: {resolved}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
