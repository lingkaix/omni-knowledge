#!/usr/bin/env python3
"""Admit the actual GUI gate/bundle lifecycle with isolated Linux tool effects.

Run this file with a committed subject and a new evidence directory.
No controller source is imported, rewritten, or used to derive an oracle.
This is instrument admission, not macOS, GUI, signing, or product qualification.
"""

import argparse
import ctypes
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tarfile
import time
import unittest


HERE = Path(__file__).resolve().parent
OPTIONS = None


def digest(path):
    """Return the SHA-256 identity of a fixture or opaque subject file."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def processes():
    """Read Linux process identities and ancestry without reading environments."""
    result = {}
    for directory in Path("/proc").iterdir():
        if not directory.name.isdigit():
            continue
        try:
            fields = (directory / "stat").read_text().rsplit(")", 1)[1].split()
            result[int(directory.name)] = {"state": fields[0], "ppid": int(fields[1]),
                                          "pgid": int(fields[2]), "sid": int(fields[3]), "start": fields[19]}
        except (FileNotFoundError, ProcessLookupError):
            pass
    return result


def owned_processes():
    """Return only descendants of this sequential, child-subreaper test driver."""
    rows = processes()
    owned = {os.getpid()}
    while True:
        added = {pid for pid, row in rows.items() if row["ppid"] in owned} - owned
        if not added:
            return {pid: rows[pid] for pid in owned if pid != os.getpid()}
        owned.update(added)


def cleanup(gate):
    """Kill only exact fixture descendants with pidfds, then reap and verify.

    Subreaping retains ownership after OLD orphans a nested session.
    No cleanup signal is sent until deciding observations have been recorded.
    Return the surviving live identities; an empty result means cleanup settled.
    """
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        rows = owned_processes()
        for pid, row in rows.items():
            try:
                fd = os.pidfd_open(pid)
                try:
                    current = processes().get(pid)
                    if current and current["start"] == row["start"]:
                        signal.pidfd_send_signal(fd, signal.SIGKILL)
                finally:
                    os.close(fd)
            except ProcessLookupError:
                pass
        gate.poll()
        while True:
            try:
                if os.waitpid(-1, os.WNOHANG)[0] == 0:
                    break
            except ChildProcessError:
                break
        if not owned_processes():
            return {}
        time.sleep(0.02)
    return owned_processes()


class BundleLifecycle(unittest.TestCase):
    """Check one real ownership crossing with success and failure controls."""

    def run_case(self, route, mode):
        """Run an isolated committed subject; retain observations even on failure.

        Route selects only the nested Cargo or supplier effect.
        Mode selects normal success, deliberate failure, outer timeout, or SIGINT.
        The original failure remains in evidence if final cleanup also fails.
        """
        case = OPTIONS.evidence_dir / (route + "-" + mode)
        case.mkdir()
        subject = case / "subject"
        shutil.copytree(OPTIONS.snapshot, subject)
        tools = case / "tools"
        tools.mkdir()
        for tool in ("cargo", "rustc", "ffmpeg", "ffprobe", "uname", "codesign", "git", "plutil"):
            shutil.copyfile(HERE / "standin.py", tools / tool)
            (tools / tool).chmod(0o755)
        supplier = subject / "tools/kernel-probe/prepare-dictionary.py"
        supplier.parent.mkdir(parents=True)
        shutil.copyfile(HERE / "standin.py", supplier)
        platform = case / "platform"
        platform.mkdir()
        (platform / "sitecustomize.py").write_text(
            '"""Supply only the unavailable host-platform prerequisite."""\n'
            'import platform\nplatform.system = lambda: "Darwin"\nplatform.machine = lambda: "arm64"\n')
        for name in ("home", "tmp", "target", "cargo-home"):
            (case / name).mkdir()
        env = {"PATH": str(tools) + ":/usr/bin:/bin", "HOME": str(case / "home"),
               "TMPDIR": str(case / "tmp"), "LANG": "C.UTF-8", "PYTHONPATH": str(platform),
               "PYTHONDONTWRITEBYTECODE": "1", "CARGO_HOME": str(case / "cargo-home"),
               "CARGO_TARGET_DIR": str(case / "target"), "CI": "true", "GITHUB_ACTIONS": "true",
               "PROCESS_CASE": str(case), "PROCESS_ROUTE": route, "PROCESS_MODE": mode,
               "PROCESS_SUBJECT_COMMIT": OPTIONS.commit, "PROCESS_DRIVER_PID": str(os.getpid())}
        command = [sys.executable, "scripts/gate.py", "gui", "--evidence-dir", str(case / "gate-evidence"),
                   "--timeout-seconds", "2" if mode == "timeout" else "15"]
        observed = {"command": command, "cwd": str(subject), "environment": env, "route": route, "mode": mode}
        gate = None
        try:
            with (case / "outer.log").open("wb") as output:
                gate = subprocess.Popen(command, cwd=subject, env=env, stdout=output, stderr=subprocess.STDOUT,
                                        start_new_session=True)
                deadline = time.monotonic() + 20
                while gate.poll() is None and not (case / "ready").exists() and time.monotonic() < deadline:
                    time.sleep(0.02)
                if (case / "ready").exists():
                    observed["ready"] = json.loads((case / "ready").read_text())
                    observed["at_ready"] = owned_processes()
                    if mode == "interrupt":
                        observed["signal"] = {"pid": gate.pid, "signal": "SIGINT", "time": time.monotonic()}
                        gate.send_signal(signal.SIGINT)
                try:
                    observed["returncode"] = gate.wait(timeout=max(0.1, deadline - time.monotonic()))
                    observed["terminal_time"] = time.monotonic()
                except subprocess.TimeoutExpired:
                    self.fail("outer gate did not reach a terminal outcome within 20 seconds")
                observed["at_terminal"] = owned_processes()
                if mode in ("timeout", "interrupt"):
                    (case / "release").touch()
                    observation_deadline = time.monotonic() + 0.5
                    while not (case / "delayed-write").exists() and time.monotonic() < observation_deadline:
                        time.sleep(0.01)
                    observed["delayed_write"] = (case / "delayed-write").exists()
                    observed["after_release"] = owned_processes()
            events = [json.loads(line) for line in (case / "events.jsonl").read_text().splitlines()]
            selected = [row for row in events if row["event"] == "selected"]
            self.assertTrue(selected, "nested selected tool never ran; prerequisite failure is not admission")
            chain = selected[0]["ancestors"]
            self.assertTrue(any(row["pid"] == gate.pid and "gui" in row["argv"] for row in chain),
                            "selected tool is not owned by the actual outer GUI gate")
            self.assertTrue(any("bundle-build" in row["argv"] for row in chain), "real build delegation absent")
            self.assertTrue(any(any(Path(arg).name == "bundle-macos.sh" for arg in row["argv"]) for row in chain),
                            "real bundle script absent")
            self.assertFalse(any(row["event"] == "signing" for row in events),
                             "the explicit CI unsigned path must not sign the app")
            self.assertEqual({path.name: digest(path) for path in (subject / "scripts").glob("*") if path.is_file()},
                             {path.name: digest(path) for path in (OPTIONS.snapshot / "scripts").glob("*") if path.is_file()},
                             "the actual subject scripts must remain unchanged")
            output_text = (case / "outer.log").read_text()
            # The shell exit status is the public terminal outcome; wording is not contractual.
            if mode == "success":
                self.assertEqual(observed["returncode"], 0, output_text)
            else:
                self.assertNotEqual(observed["returncode"], 0, "selected failure reported success")
            if mode in ("timeout", "interrupt"):
                self.assertIn("ready", observed, "nested long-running tool never reached its barrier")
                live = {pid: row for pid, row in observed["at_terminal"].items() if row["state"] != "Z"}
                observed["settlement_assertion_reached"] = True
                self.assertEqual({"live_owned": live, "delayed_write": observed["delayed_write"]},
                                 {"live_owned": {}, "delayed_write": False},
                                 "terminal gate failure must settle owned descendants before subsequent writes")
            observed["verdict"] = "PASS (instrument admission only)"
        except BaseException as error:
            observed["verdict"] = "FAIL"
            observed["failure"] = str(error)
            raise
        finally:
            if gate is not None:
                observed["cleanup_remaining"] = cleanup(gate)
            observed["subject_hashes_after"] = {str(path.relative_to(subject)): digest(path)
                                                 for path in (subject / "scripts").glob("*") if path.is_file()}
            (case / "observations.json").write_text(json.dumps(observed, indent=2) + "\n")
            if observed.get("cleanup_remaining"):
                raise RuntimeError("fixture cleanup failed; retained identities in " + str(case))

    def test_controls(self):
        """Require success and deliberate failure through each selected route."""
        for route in ("cargo", "supplier"):
            for mode in ("success", "failure"):
                with self.subTest(route=route, mode=mode):
                    self.run_case(route, mode)

    def test_nested_settlement(self):
        """Reject living descendants and delayed writes on timeout and SIGINT."""
        for route in ("cargo", "supplier"):
            for mode in ("timeout", "interrupt"):
                with self.subTest(route=route, mode=mode):
                    self.run_case(route, mode)


def main():
    """Export an immutable committed subject and execute ordinary unittest cases.

    The evidence directory must not exist, so results cannot overwrite an earlier run.
    Linux procfs, child-subreaping, and pidfds are required and never silently skipped.
    """
    global OPTIONS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--subject-repo", type=Path, required=True)
    parser.add_argument("--subject-ref", required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    OPTIONS, unittest_args = parser.parse_known_args()
    OPTIONS.evidence_dir = OPTIONS.evidence_dir.resolve()
    OPTIONS.subject_repo = OPTIONS.subject_repo.resolve()
    OPTIONS.evidence_dir.mkdir(parents=True)
    if sys.platform != "linux" or not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        parser.error("Linux procfs and Python pidfd APIs are required for exact fixture cleanup")
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:  # Linux PR_SET_CHILD_SUBREAPER.
        raise OSError(ctypes.get_errno(), "cannot retain ownership of orphaned fixture descendants")
    commit = subprocess.check_output(["git", "-C", str(OPTIONS.subject_repo), "rev-parse", OPTIONS.subject_ref + "^{commit}"], text=True).strip()
    OPTIONS.commit = commit
    command = ["git", "-C", str(OPTIONS.subject_repo), "archive", commit,
               "scripts", "Cargo.toml", "Cargo.lock", "LICENSE", "assets/brand",
               "assets/vendor/highlight-LICENSE", "assets/vendor/lucide-LICENSE", "assets/vendor/mermaid-LICENSE",
               "assets/foundations/OKF-LICENSE"]
    archive = subprocess.check_output(command)
    OPTIONS.snapshot = OPTIONS.evidence_dir / "snapshot"
    OPTIONS.snapshot.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive)) as source:
        source.extractall(OPTIONS.snapshot, filter="data")
    metadata = {"subject_commit": commit, "export_command": command, "archive_sha256": hashlib.sha256(archive).hexdigest(),
                "instrument_command": [sys.executable, *sys.argv], "python": sys.version,
                "instrument_hashes": {path.name: digest(path) for path in HERE.glob("*.py")},
                "subject_hashes": {str(path.relative_to(OPTIONS.snapshot)): digest(path)
                                   for path in (OPTIONS.snapshot / "scripts").glob("*") if path.is_file()},
                "scope": "Linux synthetic instrument admission; no native/product/signing qualification"}
    (OPTIONS.evidence_dir / "identity.json").write_text(json.dumps(metadata, indent=2) + "\n")
    unittest.main(argv=[sys.argv[0], *unittest_args], verbosity=2)


if __name__ == "__main__":
    main()
