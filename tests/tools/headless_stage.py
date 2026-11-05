#!/usr/bin/env python3
"""Real-filesystem checks for staging a headless archive from a clean checkout."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "bundle-headless.py"
SPEC = importlib.util.spec_from_file_location("headless_stage_subject", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SUBJECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUBJECT)


class StageDirectoryTests(unittest.TestCase):
    def test_missing_parent_is_created(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory) / "dist" / "linux-x86_64"
            SUBJECT.create_stage_directory(stage)
            self.assertTrue(stage.is_dir())
            self.assertEqual(list(stage.iterdir()), [])

    def test_existing_file_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory) / "existing"
            stage.write_bytes(b"owned competing file")
            with self.assertRaises(ValueError):
                SUBJECT.create_stage_directory(stage)
            self.assertEqual(stage.read_bytes(), b"owned competing file")

    def test_existing_directory_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory) / "existing"
            stage.mkdir()
            child = stage / "unknown"
            child.write_bytes(b"preserve")
            with self.assertRaises(ValueError):
                SUBJECT.create_stage_directory(stage)
            self.assertEqual(child.read_bytes(), b"preserve")

    def test_dangling_stage_link_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stage = root / "link"
            stage.symlink_to(root / "absent", target_is_directory=True)
            with self.assertRaises(ValueError):
                SUBJECT.create_stage_directory(stage)
            self.assertTrue(stage.is_symlink())
            self.assertFalse((root / "absent").exists())


class GateAccountingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        path = SCRIPT.with_name("gate.py")
        spec = importlib.util.spec_from_file_location("gate_accounting_subject", path)
        assert spec is not None and spec.loader is not None
        cls.gate = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.gate)

    def test_named_and_unnamed_rustdoc_compile_results_keep_exact_identity(self) -> None:
        for name in (
            "crates/example/src/lib.rs - (line 8)",
            "crates/example/src/lib.rs - Scope (line 131)",
            "crates/example/src/tracking.rs - tracking::Store<P>::reconcile_watched (line 382)",
        ):
            with self.subTest(name=name):
                self.assertEqual(self.gate.canonical_test_name(name + " - compile"), name)
                self.assertEqual(self.gate.canonical_test_name(name), name)
        self.assertEqual(self.gate.canonical_test_name("caller - compile"), "caller - compile")

    def test_compile_labels_do_not_hide_missing_or_renamed_cases(self) -> None:
        from collections import Counter
        name = "crates/example/src/lib.rs - Scope (line 131)"
        ignored = Counter()
        extra_output = ""
        if self.gate.platform.system() == "Darwin":
            ignored["actual_local_asr_recognizes_known_speech"] = 1
            extra_output = "test actual_local_asr_recognizes_known_speech ... ignored\n"
        inventory = Counter({name: 1}) + ignored
        self.gate.check_tests(inventory, ignored, f"test {name} - compile ... ok\n" + extra_output)
        for output in (extra_output, f"test {name.replace('Scope', 'Other')} - compile ... ok\n" + extra_output):
            with self.assertRaises(self.gate.GateError):
                self.gate.check_tests(inventory, ignored, output)


if __name__ == "__main__":
    unittest.main()
