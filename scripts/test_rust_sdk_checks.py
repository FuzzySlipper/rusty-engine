#!/usr/bin/env python3
"""Focused negative proofs for the complete SDK and rolling revision checks."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPTS = Path(__file__).resolve().parent


def load_script(module_name: str, filename: str):
    specification = importlib.util.spec_from_file_location(module_name, SCRIPTS / filename)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


freshness = load_script(
    "check_downstream_engine_freshness_module", "check_downstream_engine_freshness.py"
)
sdk_coverage = load_script("check_rust_sdk_coverage_module", "check-rust-sdk-coverage.py")


class RustSdkCoverageTests(unittest.TestCase):
    def test_current_workspace_is_complete(self) -> None:
        self.assertEqual(sdk_coverage.find_violations(sdk_coverage.load_metadata()), [])

    def test_new_public_library_fails_until_dependency_export_and_index_exist(self) -> None:
        metadata = {
            "workspace_members": ["path+file:///probe#new-public-library@0.1.0"],
            "packages": [
                {
                    "id": "path+file:///probe#new-public-library@0.1.0",
                    "name": "new-public-library",
                    "targets": [{"kind": ["lib"]}],
                }
            ],
        }
        with (
            mock.patch.object(sdk_coverage, "facade_dependencies", return_value={}),
            mock.patch.object(sdk_coverage, "facade_reexports", return_value=set()),
            mock.patch.object(sdk_coverage, "documented_capabilities", return_value={}),
        ):
            violations = sdk_coverage.find_violations(metadata)
        self.assertIn(
            "missing unconditional facade dependencies: new-public-library", violations
        )
        self.assertIn("missing exact namespace re-exports: new_public_library", violations)
        self.assertIn("capability index is missing: new-public-library", violations)

    def test_freshness_accepts_current_and_rejects_stale_lock(self) -> None:
        current = "1" * 40
        stale = "2" * 40
        with tempfile.TemporaryDirectory(prefix="rusty-engine-freshness-") as temporary:
            lockfile = Path(temporary) / "Cargo.lock"
            lockfile.write_text(
                """version = 4

[[package]]
name = "rusty-engine"
version = "0.1.0"
source = "git+https://github.com/FuzzySlipper/rusty-engine?branch=main#"""
                + current
                + '"\n',
                encoding="utf-8",
            )
            self.assertEqual(
                freshness.check_freshness(
                    lockfile, freshness.DEFAULT_REPOSITORY, "main", current
                ),
                (current, current),
            )
            with self.assertRaisesRegex(RuntimeError, "lock is stale"):
                freshness.check_freshness(
                    lockfile, freshness.DEFAULT_REPOSITORY, "main", stale
                )

    def test_manifest_requires_only_the_complete_rolling_facade(self) -> None:
        with tempfile.TemporaryDirectory(prefix="rusty-engine-manifest-") as temporary:
            manifest = Path(temporary) / "Cargo.toml"
            manifest.write_text(
                """[dependencies]
rusty-engine = { git = "https://github.com/FuzzySlipper/rusty-engine", branch = "main" }
""",
                encoding="utf-8",
            )
            freshness.validate_manifest(
                manifest, freshness.DEFAULT_REPOSITORY, "main"
            )
            manifest.write_text(
                """[dependencies]
rusty-engine = { git = "https://github.com/FuzzySlipper/rusty-engine", branch = "main" }
render-model = { git = "https://github.com/FuzzySlipper/rusty-engine", branch = "main" }
""",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "exactly one dependency"):
                freshness.validate_manifest(
                    manifest, freshness.DEFAULT_REPOSITORY, "main"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
