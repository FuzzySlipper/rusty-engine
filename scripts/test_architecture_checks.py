#!/usr/bin/env python3
"""Focused positive and negative tests for workspace dependency boundaries and environment reads."""

from __future__ import annotations

import re
import sys
from pathlib import Path
import unittest

sys.dont_write_bytecode = True
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

    def test_host_adapter_may_execute_an_authoritative_owner_model(self) -> None:
        metadata = metadata_fixture(
            ["content-store", "content-store-host", "entity-state"],
            [
                ("content-store-host", "content-store", None, "content_store"),
                ("content-store", "entity-state", None, "entity_state"),
            ],
        )
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

    def test_render_authority_paths_are_rejected(self) -> None:
        metadata = metadata_fixture(
            [
                "content-store",
                "entity-state",
                "render-wgpu",
                "render-model",
                "render-presentation",
                "render-projection",
            ],
            [
                ("entity-state", "render-projection", None, "projection"),
                ("render-model", "render-wgpu", None, "backend"),
                ("render-presentation", "render-projection", None, "projection"),
            ],
        )
        rendered = "\n".join(dependency_boundary_check.find_violations(metadata))
        self.assertIn("authoritative owner reverse-depends on render-projection", rendered)
        self.assertIn("renderer-neutral model reaches authority", rendered)

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

    def test_only_the_owner_may_depend_on_an_owned_external_crate(self) -> None:
        metadata = metadata_fixture(["render-audio", "csharp-product-runtime"], [])
        packages = {package["name"]: package for package in metadata["packages"]}
        packages["render-audio"]["dependencies"] = [{"name": "kira", "kind": None}]
        packages["csharp-product-runtime"]["dependencies"] = [
            {"name": "render-audio", "kind": None},
            {"name": "cpal", "kind": "dev"},
        ]
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

        packages["csharp-product-runtime"]["dependencies"].append(
            {"name": "kira", "kind": "build"}
        )
        self.assertEqual(
            dependency_boundary_check.find_violations(metadata),
            ["csharp-product-runtime depends on kira, which only render-audio may depend on"],
        )

    def test_only_render_wgpu_may_depend_on_wgpu(self) -> None:
        metadata = metadata_fixture(["render-wgpu", "render-presentation"], [])
        packages = {package["name"]: package for package in metadata["packages"]}
        packages["render-wgpu"]["dependencies"] = [
            {"name": "wgpu", "kind": None},
            {"name": "render-presentation", "kind": "dev"},
        ]
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

        packages["render-presentation"]["dependencies"] = [{"name": "wgpu-types", "kind": None}]
        self.assertEqual(
            dependency_boundary_check.find_violations(metadata),
            ["render-presentation depends on wgpu-types, which only render-wgpu may depend on"],
        )

    def test_only_host_facing_crates_may_generate_typescript(self) -> None:
        metadata = metadata_fixture(["runtime-ui", "product-host", "render-host-contracts"], [])
        packages = {package["name"]: package for package in metadata["packages"]}
        packages["runtime-ui"]["dependencies"] = [{"name": "ts-rs", "kind": None}]
        packages["product-host"]["dependencies"] = [{"name": "ts-rs", "kind": None}]
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

        packages["render-host-contracts"]["dependencies"] = [{"name": "ts-rs", "kind": None}]
        self.assertEqual(
            dependency_boundary_check.find_violations(metadata),
            [
                "render-host-contracts depends on ts-rs, which only product-host, "
                "runtime-diagnostics, runtime-input, runtime-ui may depend on"
            ],
        )

    def test_render_wgpu_may_not_write_json(self) -> None:
        metadata = metadata_fixture(["render-wgpu", "render-export"], [])
        packages = {package["name"]: package for package in metadata["packages"]}
        packages["render-export"]["dependencies"] = [{"name": "serde_json", "kind": None}]
        packages["render-wgpu"]["dependencies"] = [{"name": "serde_json", "kind": "dev"}]
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

        packages["render-wgpu"]["dependencies"] = [{"name": "serde_json", "kind": None}]
        self.assertEqual(
            dependency_boundary_check.find_violations(metadata),
            ["render-wgpu may not depend on serde_json"],
        )

    def test_render_wgpu_depends_only_on_the_render_vocabulary(self) -> None:
        metadata = metadata_fixture(
            ["render-wgpu", "render-model", "render-video", "product-host", "render-stream"],
            [
                ("render-wgpu", "render-model", None, "render_model"),
                ("render-wgpu", "render-video", None, "render_video"),
                ("render-wgpu", "render-stream", "dev", "render_stream"),
                ("render-stream", "render-wgpu", None, "render_wgpu"),
            ],
        )
        self.assertEqual(dependency_boundary_check.find_violations(metadata), [])

        metadata = metadata_fixture(
            ["render-wgpu", "render-model", "product-host"],
            [
                ("render-wgpu", "render-model", None, "render_model"),
                ("render-wgpu", "product-host", None, "host"),
            ],
        )
        self.assertEqual(
            dependency_boundary_check.find_violations(metadata),
            [
                "render-wgpu depends on product-host, outside the renderer's "
                "workspace dependencies"
            ],
        )


# Crates that must not read the environment to select behaviour (owner
# decision 2026-09-30, #8954): settings belong in the product manifest, launch
# arguments or a config file, where they can be found.
ENVIRONMENT_CHECKED_CRATES = (
    "csharp-product-runtime",
    "desktop-shell",
    "product-host",
    "render-audio",
    "render-export",
    "render-stream",
    "render-wgpu",
    "rusty-cli",
)
# Platform conventions these crates may read, and why.
ALLOWED_ENVIRONMENT_READS = {
    "PATH": "finding executables (dotnet, Chromium)",
    "HOME": "the default cache location",
    "XDG_CACHE_HOME": "the platform's cache location",
    "DOTNET_ROOT": ".NET's own convention for locating its runtime",
    "WAYLAND_DISPLAY": "which display server the window opens on",
    "WGPU_BACKEND": "wgpu's own adapter override, honoured before choosing one",
}
# The one admitted form of an environment read: `env::var("NAME")` or
# `env::var_os("NAME")` with an allowed literal name. Every other mention of
# std::env's readers is refused, so no import, alias, glob, function value or
# FFI call can read the environment out of the check's sight.
ALLOWED_READ = re.compile(r'env\s*::\s*var(?:_os)?\s*\(\s*"([A-Z_0-9]+)"\s*\)')
READER_MENTION = re.compile(r"env\s*::\s*(?:\{[^}]*\}|vars?(?:_os)?\b)")
USE_STATEMENT = re.compile(r"\buse\s+[^;]*;", re.DOTALL)
GETENV = re.compile(r"\bgetenv\b")


def environment_violations(text: str) -> list[str]:
    """Environment reads in Rust source that could select behaviour."""
    violations = []
    admitted = set()
    for match in ALLOWED_READ.finditer(text):
        if match.group(1) in ALLOWED_ENVIRONMENT_READS:
            admitted.add(match.start())
        else:
            violations.append(match.group(0))
    for match in READER_MENTION.finditer(text):
        if match.start() not in admitted and not ALLOWED_READ.match(text, match.start()):
            violations.append(match.group(0))
    for statement in USE_STATEMENT.finditer(text):
        normalized = " ".join(statement.group(0).split())
        # A `use` that reaches std::env may import the module itself, nothing
        # from it, and may neither glob nor rename.
        if re.search(r"\benv\b", normalized) and re.search(r"\*|\bas\b|env\s*::", normalized):
            violations.append(normalized)
    violations.extend(match.group(0) for match in GETENV.finditer(text))
    return violations


class EnvironmentReadTests(unittest.TestCase):
    def test_behaviour_is_not_selected_by_environment_variables(self) -> None:
        refused = [
            f"{source.relative_to(REPO_ROOT)}: {violation}"
            for crate in ENVIRONMENT_CHECKED_CRATES
            for source in sorted((REPO_ROOT / "rust" / "crates" / crate / "src").rglob("*.rs"))
            for violation in environment_violations(source.read_text(encoding="utf-8"))
        ]
        self.assertEqual(
            refused,
            [],
            "select behaviour through the product manifest, a launch argument or a config file",
        )

    def test_the_check_sees_every_way_to_read_a_behaviour_variable(self) -> None:
        self.assertNotIn("RUSTY_RENDER_OUTPUT", ALLOWED_ENVIRONMENT_READS)
        for text in [
            'let output = std::env::var_os("RUSTY_RENDER_OUTPUT");',
            "let output = env::var(OUTPUT_VARIABLE);",
            'let path = std::env::var_os("PATH").or(env::var_os(FALLBACK));',
            'use std::env::var;\nlet output = var("RUSTY_RENDER_OUTPUT");',
            'use std::env::{self, var_os};\nlet output = var_os("RUSTY_RENDER_OUTPUT");',
            'use std::{\n    env::var_os as read,\n    fs,\n};\nlet output = read("RUSTY_RENDER_OUTPUT");',
            'use std::env as environment;\nlet output = environment::var("RUSTY_RENDER_OUTPUT");',
            'use std::env::*;\nlet output = var("RUSTY_RENDER_OUTPUT");',
            'use std::env;\nlet read = env::var;\nlet output = read("RUSTY_RENDER_OUTPUT");',
            'use std::env::{self as environment};\nlet output = environment::var("RUSTY_RENDER_OUTPUT");',
            'let output = unsafe { libc::getenv(name.as_ptr()) };',
            "for (name, value) in std::env::vars() {}",
        ]:
            self.assertNotEqual(environment_violations(text), [], text)

    def test_platform_conventions_and_plain_env_imports_pass(self) -> None:
        text = (
            "use std::{env, fs};\n"
            'let path = env::var_os("PATH");\n'
            'let home = std::env::var_os("HOME");\n'
            "let exe = env::current_exe();\n"
        )
        self.assertEqual(environment_violations(text), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
