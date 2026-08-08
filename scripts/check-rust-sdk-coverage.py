#!/usr/bin/env python3
"""Require the Rust facade to preserve every workspace library namespace."""

from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
FACADE_NAME = "rusty-engine"
FACADE_ROOT = REPO_ROOT / "rust" / "crates" / FACADE_NAME
CAPABILITY_INDEX = REPO_ROOT / "docs" / "rust-sdk-capabilities.md"


def load_metadata(root: Path = REPO_ROOT) -> dict[str, Any]:
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--no-deps"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def public_libraries(metadata: dict[str, Any]) -> dict[str, str]:
    workspace_ids = set(metadata["workspace_members"])
    libraries: dict[str, str] = {}
    for package in metadata["packages"]:
        if package["id"] not in workspace_ids or package["name"] == FACADE_NAME:
            continue
        if any("lib" in target["kind"] for target in package["targets"]):
            libraries[package["name"]] = package["name"].replace("-", "_")
    return libraries


def facade_dependencies() -> dict[str, Any]:
    with (FACADE_ROOT / "Cargo.toml").open("rb") as source:
        return tomllib.load(source).get("dependencies", {})


def facade_reexports() -> set[str]:
    source = (FACADE_ROOT / "src" / "lib.rs").read_text(encoding="utf-8")
    return set(re.findall(r"^pub use ([a-z][a-z0-9_]*);$", source, re.MULTILINE))


def documented_capabilities() -> dict[str, str]:
    source = CAPABILITY_INDEX.read_text(encoding="utf-8")
    rows = re.findall(
        r"^\| `([a-z][a-z0-9-]*)` \| `rusty_engine::([a-z][a-z0-9_]*)` \|$",
        source,
        re.MULTILINE,
    )
    return dict(rows)


def find_violations(metadata: dict[str, Any]) -> list[str]:
    libraries = public_libraries(metadata)
    dependencies = facade_dependencies()
    reexports = facade_reexports()
    documented = documented_capabilities()
    violations: list[str] = []

    expected_packages = set(libraries)
    dependency_packages = set(dependencies)
    if missing := sorted(expected_packages - dependency_packages):
        violations.append(f"missing unconditional facade dependencies: {', '.join(missing)}")
    if extra := sorted(dependency_packages - expected_packages):
        violations.append(f"facade dependencies without public library targets: {', '.join(extra)}")

    for package in sorted(expected_packages & dependency_packages):
        specification = dependencies[package]
        if specification is not True and not (
            isinstance(specification, dict)
            and specification.get("workspace") is True
            and specification.get("optional") is not True
            and "target" not in specification
        ):
            violations.append(f"{package} is not an unconditional workspace dependency")

    expected_namespaces = set(libraries.values())
    if missing := sorted(expected_namespaces - reexports):
        violations.append(f"missing exact namespace re-exports: {', '.join(missing)}")
    if extra := sorted(reexports - expected_namespaces):
        violations.append(f"unexpected facade re-exports: {', '.join(extra)}")

    if documented != libraries:
        missing = sorted(expected_packages - set(documented))
        extra = sorted(set(documented) - expected_packages)
        mismatched = sorted(
            package
            for package in expected_packages & set(documented)
            if documented[package] != libraries[package]
        )
        if missing:
            violations.append(f"capability index is missing: {', '.join(missing)}")
        if extra:
            violations.append(f"capability index has stale entries: {', '.join(extra)}")
        if mismatched:
            violations.append(f"capability index has wrong namespaces: {', '.join(mismatched)}")

    return violations


def main() -> int:
    try:
        metadata = load_metadata()
        violations = find_violations(metadata)
    except (OSError, subprocess.CalledProcessError, KeyError, json.JSONDecodeError) as error:
        print(f"Rust SDK coverage check failed: {error}", file=sys.stderr)
        return 2

    if violations:
        print("Rust SDK coverage violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1

    print(
        "Rust SDK coverage passed: "
        f"{len(public_libraries(metadata))} public libraries preserve exact namespaces"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
