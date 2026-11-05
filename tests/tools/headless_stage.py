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


if __name__ == "__main__":
    unittest.main()
