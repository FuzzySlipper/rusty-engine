#!/usr/bin/env python3
"""Focused positive and negative tests for the lightweight architecture checks."""

from __future__ import annotations

from contextlib import redirect_stdout
import io
import os
import sys
import tempfile
from pathlib import Path
import unittest
from unittest import mock

sys.dont_write_bytecode = True
import code_map_freshness
import dependency_boundary_check


REPO_ROOT = Path(__file__).resolve().parents[1]


def metadata_fixture(
    package_names: list[str],
    edges: list[tuple[str, str, str | None, str]],
    root: Path | None = None,
) -> dict[str, object]:
    fixture_root = root or Path("/fixture")
    package_ids = {
        name: f"path+file://{fixture_root}/rust/crates/{name}#0.1.0" for name in package_names
    }
    node_dependencies: dict[str, list[dict[str, object]]] = {
        name: [] for name in package_names
    }
    for source, target, kind, alias in edges:
        node_dependencies[source].append(
            {
                "name": alias,
                "pkg": package_ids[target],
                "dep_kinds": [{"kind": kind, "target": None}],
            }
        )
    return {
        "workspace_members": list(package_ids.values()),
        "packages": [
            {
                "id": package_id,
                "name": name,
                "manifest_path": str(fixture_root / "rust" / "crates" / name / "Cargo.toml"),
            }
            for name, package_id in package_ids.items()
        ],
        "resolve": {
            "nodes": [
                {"id": package_ids[name], "deps": node_dependencies[name]}
                for name in package_names
            ]
        },
    }


class DependencyBoundaryTests(unittest.TestCase):
    def test_current_workspace_graph_is_accepted(self) -> None:
        metadata = dependency_boundary_check.load_metadata(REPO_ROOT)
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

    def test_render_projection_may_observe_entity_spatial_voxel_and_service_facts(self) -> None:
        metadata = metadata_fixture(
            [
                "core-ids",
                "engine-spatial",
                "entity-state",
                "render-model",
                "render-projection",
                "svc-mesh",
                "voxel-object-runtime",
            ],
            [
                ("engine-spatial", "core-ids", None, "core_ids"),
                ("engine-spatial", "entity-state", None, "entity_state"),
                ("render-projection", "engine-spatial", None, "spatial"),
                ("render-projection", "entity-state", None, "entities"),
                ("render-projection", "render-model", None, "model"),
                ("render-projection", "svc-mesh", None, "meshing"),
                ("render-projection", "voxel-object-runtime", None, "voxel_objects"),
                ("svc-mesh", "core-ids", None, "ids"),
                ("voxel-object-runtime", "svc-mesh", None, "mesh_service"),
            ],
        )
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

    def test_renamed_direct_dependency_cannot_hide_core_inversion(self) -> None:
        metadata = metadata_fixture(
            ["core-assets", "entity-state"],
            [("core-assets", "entity-state", None, "renamed_entity_facts")],
        )
        violations = dependency_boundary_check.find_violations(metadata)
        self.assertTrue(
            any(
                "core-assets -> entity-state" in violation
                and "core foundation" in violation
                for violation in violations
            )
        )

    def test_transitive_service_inversion_reports_the_complete_path(self) -> None:
        metadata = metadata_fixture(
            ["content-store", "svc-spatial", "svc-volume"],
            [
                ("svc-volume", "svc-spatial", None, "spatial"),
                ("svc-spatial", "content-store", None, "content"),
            ],
        )
        violations = dependency_boundary_check.find_violations(metadata)
        self.assertTrue(
            any(
                "svc-volume -> svc-spatial -> content-store" in violation
                and "service mechanism" in violation
                for violation in violations
            )
        )

    def test_inspector_and_render_authority_paths_are_rejected(self) -> None:
        metadata = metadata_fixture(
            [
                "engine-inspector",
                "entity-state",
                "gameplay-mechanics",
                "renderer-host",
                "render-model",
                "render-presentation",
                "render-projection",
            ],
            [
                ("entity-state", "render-projection", None, "projection"),
                ("gameplay-mechanics", "engine-inspector", None, "inspection"),
                ("render-model", "gameplay-mechanics", None, "mechanics"),
                ("render-model", "renderer-host", None, "browser_host"),
                ("render-presentation", "render-projection", None, "projection"),
                ("render-projection", "gameplay-mechanics", None, "mechanics"),
            ],
        )
        rendered = "\n".join(dependency_boundary_check.find_violations(metadata))
        self.assertIn("authoritative owner reverse-depends on render-projection", rendered)
        self.assertIn("ordinary workspace package reaches the engine-inspector leaf", rendered)
        self.assertIn("renderer-neutral model reaches authority", rendered)
        self.assertIn("render-projection reaches gameplay authority", rendered)

    def test_dev_edges_are_ignored_but_build_edges_are_enforced(self) -> None:
        dev_metadata = metadata_fixture(
            ["core-assets", "entity-state"],
            [("core-assets", "entity-state", "dev", "test_facts")],
        )
        self.assertEqual(dependency_boundary_check.find_violations(dev_metadata), [])

        build_metadata = metadata_fixture(
            ["core-assets", "entity-state"],
            [("core-assets", "entity-state", "build", "generated_facts")],
        )
        self.assertNotEqual(dependency_boundary_check.find_violations(build_metadata), [])


class CodeMapFreshnessTests(unittest.TestCase):
    def test_current_workspace_has_complete_resolving_owner_maps(self) -> None:
        metadata = code_map_freshness.load_metadata(REPO_ROOT)
        self.assertEqual(code_map_freshness.find_advisories(REPO_ROOT, metadata), [])

    def test_missing_assignment_stale_crate_and_unresolved_path_are_advisory(self) -> None:
        with tempfile.TemporaryDirectory(prefix="rusty-code-map-test-") as temporary:
            root = Path(temporary)
            (root / "docs" / "code-map").mkdir(parents=True)
            for name in ("core-a", "core-b", "stale-owner"):
                (root / "rust" / "crates" / name).mkdir(parents=True)
            (root / "docs" / "code-map" / "foundations.md").write_text(
                """# Foundations

## Primary paths

- [`core-a`](../../rust/crates/core-a)
- [`stale-owner`](../../rust/crates/stale-owner)
- [`missing`](../../missing/entry.rs)

## Acceptance gates and fixtures
""",
                encoding="utf-8",
            )
            metadata = metadata_fixture(["core-a", "core-b"], [], root)
            rendered = "\n".join(
                advisory.message for advisory in code_map_freshness.find_advisories(root, metadata)
            )
            self.assertIn("missing owner-map assignment for Cargo package core-b", rendered)
            self.assertIn("stale Cargo crate path", rendered)
            self.assertIn("unresolved primary-path reference", rendered)

    def test_github_advisory_emits_annotation_and_step_summary_without_failure(self) -> None:
        with tempfile.TemporaryDirectory(prefix="rusty-code-map-output-") as temporary:
            summary_path = Path(temporary) / "summary.md"
            output = io.StringIO()
            with mock.patch.dict(
                os.environ,
                {
                    "GITHUB_ACTIONS": "true",
                    "GITHUB_STEP_SUMMARY": str(summary_path),
                },
            ), redirect_stdout(output):
                code_map_freshness.report(
                    2,
                    [
                        code_map_freshness.Advisory(
                            "missing owner-map assignment",
                            Path("docs/code-map/example.md"),
                            12,
                        )
                    ],
                )
            self.assertIn("::warning ", output.getvalue())
            self.assertIn("file=docs/code-map/example.md", output.getvalue())
            self.assertIn("line=12", output.getvalue())
            self.assertIn("verification remains green", output.getvalue())
            self.assertIn("non-blocking issue", summary_path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
