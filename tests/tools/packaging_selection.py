#!/usr/bin/env python3
"""Layout checks for headless helper install maps.

These checks load ``scripts/bundle-headless.py`` and exercise the mapping directly.
They do not build Cargo targets or claim product or macOS acceptance.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("bundle_headless", REPO / "scripts/bundle-headless.py")
bundle = importlib.util.module_from_spec(SPEC)
assert SPEC is not None and SPEC.loader is not None
SPEC.loader.exec_module(bundle)


def processing_metadata(src_name: str) -> dict:
    """Return one knowledge-processing package with one normal bin."""
    return {
        "workspace_members": ["kp"],
        "packages": [{
            "id": "kp",
            "name": "knowledge-processing",
            "targets": [
                {
                    "name": "parser-worker",
                    "kind": ["bin"],
                    "src_path": f"/work/crates/knowledge-processing/src/bin/{src_name}",
                    "required-features": [],
                },
                {"name": "adapters", "kind": ["test"], "src_path": "/work/tests/adapters.rs"},
            ],
        }],
    }


def fsevents_metadata(src_name: str = "main.rs") -> dict:
    """Return one knowledge-local package with one normal bin."""
    return {
        "workspace_members": ["kl"],
        "packages": [{
            "id": "kl",
            "name": "knowledge-local",
            "targets": [{
                "name": "omni-fsevents",
                "kind": ["bin"],
                "src_path": f"/work/crates/knowledge-local/src/bin/omni-fsevents/{src_name}",
                "required-features": [],
            }],
        }],
    }


class InstallMapTests(unittest.TestCase):
    """Cargo names install only under the parent lookup basename."""

    def test_verified_maps_are_the_lookup_basenames(self):
        self.assertEqual(bundle.PARSER_INSTALL, {"parser-worker": "parser-worker"})
        self.assertEqual(bundle.FSEVENTS_INSTALL, {"omni-fsevents": "omni-fsevents"})
        self.assertEqual(bundle.PARSER_BASENAME, "parser-worker")
        self.assertEqual(bundle.FSEVENTS_BASENAME, "omni-fsevents")

    def test_linux_omits_fsevents_after_cargo_names_it(self):
        rows, omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents"], "x86_64-unknown-linux-gnu", None, bundle.FSEVENTS_INSTALL)
        self.assertEqual(
            [(row["cargo_name"], row["installed_name"], row["role"]) for row in rows],
            [("omni", "omni", "product")],
        )
        self.assertEqual(omitted[0]["cargo_name"], "omni-fsevents")
        self.assertIn("not FSEvents proof", omitted[0]["reason"])

    def test_apple_installs_fsevents_beside_omni(self):
        rows, omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents"], "aarch64-apple-darwin", None, bundle.FSEVENTS_INSTALL)
        self.assertEqual(omitted, [])
        self.assertEqual(
            [(row["cargo_name"], row["installed_name"], row["role"]) for row in rows],
            [("omni", "omni", "product"), ("omni-fsevents", "omni-fsevents", "fsevents-helper")],
        )

    def test_declared_parser_installs_under_the_lookup_basename(self):
        rows, _omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents", "parser-worker"],
            "x86_64-unknown-linux-gnu",
            bundle.PARSER_INSTALL,
            bundle.FSEVENTS_INSTALL,
        )
        helper = next(row for row in rows if row["role"] == "parser-helper")
        self.assertEqual((helper["cargo_name"], helper["installed_name"]), ("parser-worker", "parser-worker"))

    def test_explicit_rename_bridges_an_underscore_artifact(self):
        rows, _omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents", "parser_worker"],
            "x86_64-unknown-linux-gnu",
            {"parser_worker": "parser-worker"},
            bundle.FSEVENTS_INSTALL,
        )
        helper = next(row for row in rows if row["role"] == "parser-helper")
        self.assertEqual(helper["cargo_name"], "parser_worker")
        self.assertEqual(helper["installed_name"], "parser-worker")
        self.assertNotIn("parser_worker", {row["installed_name"] for row in rows})

    def test_unmapped_future_helper_is_refused(self):
        with self.assertRaises(ValueError):
            bundle.plan_product_bins(
                ["future-helper", "omni", "omni-fsevents"],
                "x86_64-unknown-linux-gnu",
                None,
                bundle.FSEVENTS_INSTALL,
            )

    def test_missing_fsevents_target_is_refused_on_linux(self):
        with self.assertRaises(ValueError):
            bundle.plan_product_bins(["omni"], "x86_64-unknown-linux-gnu", None, bundle.FSEVENTS_INSTALL)

    def test_missing_declared_parser_is_refused(self):
        with self.assertRaises(ValueError):
            bundle.plan_product_bins(
                ["omni", "omni-fsevents"],
                "x86_64-unknown-linux-gnu",
                bundle.PARSER_INSTALL,
                bundle.FSEVENTS_INSTALL,
            )

    def test_install_collision_is_refused(self):
        with self.assertRaises(ValueError):
            bundle.plan_product_bins(
                ["omni", "omni-fsevents", "parser-worker", "parser_worker"],
                "aarch64-apple-darwin",
                {"parser-worker": "parser-worker", "parser_worker": "parser-worker"},
                bundle.FSEVENTS_INSTALL,
            )

    def test_same_bytes_as_omni_are_refused(self):
        with self.assertRaises(ValueError):
            bundle.refuse_cloned_product([
                {"role": "product", "sha256": "abc"},
                {"role": "fsevents-helper", "sha256": "abc"},
            ])

    def test_distinct_helper_bytes_pass(self):
        bundle.refuse_cloned_product([
            {"role": "product", "sha256": "abc"},
            {"role": "parser-helper", "sha256": "def"},
            {"role": "fsevents-helper", "sha256": "fed"},
        ])

    def test_parser_source_must_be_parser_worker_rs_when_declared(self):
        target = bundle.require_parser_target(processing_metadata("parser-worker.rs"), "declared")
        self.assertEqual(target["name"], "parser-worker")
        with self.assertRaises(ValueError):
            bundle.require_parser_target(processing_metadata("parser_worker.rs"), "declared")

    def test_absent_parser_lookup_does_not_invent_a_bin(self):
        metadata = processing_metadata("parser-worker.rs")
        metadata["packages"][0]["targets"] = [
            target for target in metadata["packages"][0]["targets"] if target["kind"] != ["bin"]
        ]
        self.assertIsNone(bundle.require_parser_target(metadata, "absent"))
        with self.assertRaises(ValueError):
            bundle.require_parser_target(processing_metadata("parser-worker.rs"), "absent")

    def test_partial_parser_lookup_is_refused(self):
        with self.assertRaises(ValueError):
            bundle.require_parser_target(processing_metadata("parser-worker.rs"), "partial")

    def test_fsevents_source_must_be_the_sibling_main(self):
        target = bundle.require_fsevents_target(fsevents_metadata())
        self.assertEqual(target["name"], "omni-fsevents")
        with self.assertRaises(ValueError):
            bundle.require_fsevents_target(fsevents_metadata("other.rs"))
        empty = fsevents_metadata()
        empty["packages"][0]["targets"] = []
        with self.assertRaises(ValueError):
            bundle.require_fsevents_target(empty)

    def test_this_tree_declares_fsevents_and_not_parser_worker(self):
        self.assertTrue(bundle.fsevents_lookup_declared(REPO))
        self.assertEqual(bundle.parser_lookup_state(REPO), "absent")

    def test_linux_notice_set_is_processing_and_apple_adds_local(self):
        linux, _omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents"], "x86_64-unknown-linux-gnu", None, bundle.FSEVENTS_INSTALL)
        apple, _omitted = bundle.plan_product_bins(
            ["omni", "omni-fsevents"], "aarch64-apple-darwin", None, bundle.FSEVENTS_INSTALL)
        self.assertEqual(bundle.notice_packages(linux), ["knowledge-processing"])
        self.assertEqual(bundle.notice_packages(apple), ["knowledge-processing", "knowledge-local"])

    def test_closure_reports_a_registry_crate_missing_from_the_inventory(self):
        metadata = {
            "workspace_members": ["kp"],
            "packages": [
                {"id": "kp", "name": "knowledge-processing", "source": None},
                {"id": "pdf", "name": "pdf-extract", "version": "0.12.1",
                 "source": "registry+https://github.com/rust-lang/crates.io-index"},
                {"id": "core", "name": "knowledge-core", "version": "0.1.0", "source": None},
            ],
            "resolve": {"nodes": [
                {"id": "kp", "deps": [
                    {"pkg": "pdf", "dep_kinds": [{"kind": None}]},
                    {"pkg": "core", "dep_kinds": [{"kind": None}]},
                    {"pkg": "devonly", "dep_kinds": [{"kind": "dev"}]},
                ]},
                {"id": "pdf", "deps": []},
                {"id": "core", "deps": []},
            ]},
        }
        self.assertEqual(bundle.registry_closure(metadata, "knowledge-processing"), [("pdf-extract", "0.12.1")])
        self.assertEqual(
            bundle.missing_registry_closure(metadata, "knowledge-processing", {"packages": []}),
            ["pdf-extract@0.12.1"],
        )
        self.assertEqual(
            bundle.missing_registry_closure(
                metadata, "knowledge-processing",
                {"packages": [{"name": "pdf-extract", "version": "0.12.1"}]},
            ),
            [],
        )

    def test_non_apple_field_records_the_omission(self):
        fields = bundle.helper_fields("x86_64-unknown-linux-gnu", None, {"name": "omni-fsevents"})
        self.assertEqual(fields["parser_helper"]["status"], "not-selected")
        self.assertEqual(fields["fsevents_helper"]["status"], "omitted")
        apple = bundle.helper_fields("aarch64-apple-darwin", None, {"name": "omni-fsevents"})
        self.assertEqual(apple["fsevents_helper"]["installed_name"], "omni-fsevents")


if __name__ == "__main__":
    unittest.main()
