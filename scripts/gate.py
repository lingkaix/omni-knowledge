#!/usr/bin/env python3
"""Run the workspace CI gate with bounded commands and libtest accounting.

Only the existing real-ASR opt-in is excluded; additional ignored cases fail.
Evidence stays in temp/ci-evidence (or --evidence-dir) and contains no user data.
"""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys


class GateError(Exception):
    """Report a failed prerequisite, command, or test-accounting assertion."""


def run(command, evidence, label, timeout):
    """Run an argument array, retain output, and kill its process group on timeout.

    Return the decoded output on success; raise GateError on FAIL or TIMEOUT.
    The caller supplies a private evidence directory and a positive deadline.
    """
    print(f"RUN {label}: {' '.join(map(str, command))}", flush=True)
    log = evidence / f"{label}.log"
    with log.open("w") as output:
        process = subprocess.Popen(
            command, stdout=output, stderr=subprocess.STDOUT, start_new_session=True
        )
        try:
            try:
                code = process.wait(timeout=timeout)
                status = "PASS" if code == 0 else "FAIL"
            except subprocess.TimeoutExpired:
                code, status = 124, "TIMEOUT"
        finally:
            # Also settle descendants if the group leader exits before its children.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
    text = log.read_text(errors="replace")
    (evidence / f"{label}.json").write_text(
        json.dumps({"command": command, "status": status, "exit_code": code}) + "\n"
    )
    print(f"{status} {label} (exit {code}); {log}", flush=True)
    if status != "PASS":
        print(text[-12000:], flush=True)
        raise GateError(f"{status}: {label}")
    return text


def listed_tests(text):
    """Return a multiset of libtest names, including repeated names across targets."""
    return Counter(re.findall(r"^(.+): test$", text, re.MULTILINE))


def check_tests(inventory, ignored, output):
    """Require every collected case to finish once, with only declared ignores.

    Raise GateError for empty collection, new ignores, lost cases, or failures.
    Preserve test names instead of treating a cargo exit code as coverage proof.
    """
    if not inventory:
        raise GateError("FAIL: zero tests collected")
    allowed = Counter()
    if platform.system() == "Darwin":
        allowed["actual_local_asr_recognizes_known_speech"] = 1
    if ignored != allowed:
        raise GateError(f"FAIL: ignored inventory {ignored}; declared {allowed}")
    observed = Counter()
    actual_ignored = Counter()
    for name, outcome in re.findall(
        r"^test (.+) \.\.\. (ok|FAILED|ignored[^\n]*)$", output, re.MULTILINE
    ):
        observed[name] += 1
        if outcome.startswith("ignored"):
            actual_ignored[name] += 1
        elif outcome != "ok":
            raise GateError(f"FAIL: {name}")
    if observed != inventory or actual_ignored != ignored:
        raise GateError(
            f"FAIL: test accounting: missing={inventory - observed}, "
            f"extra={observed - inventory}, ignored={actual_ignored}"
        )
    print(f"PASS accounting: {sum(observed.values())} cases, {sum(ignored.values())} declared opt-ins")


def test_workspace(evidence, timeout, gui=False):
    """Inventory and execute workspace tests, including doctests and optional GUI.

    The gui flag selects the macOS feature configuration in addition to headless.

    Record FSEvents as pending when no adapter tests exist, never as a zero-test pass.
    Once names containing fsevent exist on macOS, require their actual completion.
    """
    cargo = ["cargo", "test", "--workspace", "--no-default-features", "--locked", "--no-fail-fast"]
    if gui:
        cargo += ["--features", "gui"]
    inventory = listed_tests(run(cargo + ["--", "--list", "--format=terse"], evidence, "test-inventory", timeout))
    ignored = listed_tests(run(cargo + ["--", "--ignored", "--list", "--format=terse"], evidence, "ignored-inventory", timeout))
    (evidence / "inventory.json").write_text(
        json.dumps({"tests": inventory, "declared_ignored": ignored}, indent=2) + "\n"
    )
    fsevents = [name for name in inventory if "fsevent" in name.lower()]
    status = "PENDING adapter tests on macOS"
    (evidence / "fsevents.json").write_text(json.dumps({"status": status, "tests": fsevents}, indent=2) + "\n")
    print(f"FSEvents: {status} ({len(fsevents)} inventoried)")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as stream:
            stream.write(f"\nFSEvents: **{status}**; {len(fsevents)} adapter cases inventoried.\n")
    output = run(cargo + ["--", "--test-threads=2", "--format=pretty"], evidence, "tests", timeout)
    check_tests(inventory, ignored, output)
    if platform.system() == "Darwin" and fsevents:
        status = "EXECUTED and accounted"
        (evidence / "fsevents.json").write_text(json.dumps({"status": status, "tests": fsevents}, indent=2) + "\n")
        print(f"FSEvents: {status}")
        if summary:
            with open(summary, "a") as stream:
                stream.write(f"\nFSEvents: **{status}**; {len(fsevents)} adapter cases passed.\n")


def smoke(binary, evidence, timeout):
    """Run the exact artifact's help and JSON health command without opening a library."""
    binary = str(Path(binary).resolve())
    run([binary, "--help"], evidence, "artifact-help", timeout)
    health = run([binary, "doctor", "--json"], evidence, "artifact-doctor", timeout)
    try:
        facts = json.loads(health)
    except json.JSONDecodeError as error:
        raise GateError("FAIL: artifact doctor did not return JSON") from error
    if not isinstance(facts, dict) or not facts.get("ffmpeg") or not facts.get("ffprobe"):
        raise GateError("FAIL: artifact cannot discover required ffmpeg/ffprobe")


def self_test(evidence):
    """Admit PASS, FAIL, TIMEOUT and accounting refusals with stable stand-ins.

    These observations test the gate instrument only, not product behavior.
    """
    run([sys.executable, "-c", "print('ready')"], evidence, "admit-pass", 5)
    for label, program, deadline, expected in [
        ("admit-fail", "raise SystemExit(7)", 5, "FAIL"),
        ("admit-timeout", "import time; print('ready', flush=True); time.sleep(30)", 1, "TIMEOUT"),
    ]:
        try:
            run([sys.executable, "-c", program], evidence, label, deadline)
        except GateError as error:
            if not str(error).startswith(expected):
                raise
        else:
            raise GateError(f"FAIL: {label} incorrectly passed")
    if "ready" not in (evidence / "admit-timeout.log").read_text():
        raise GateError("FAIL: timeout target did not reach deciding path")
    ignored = Counter()
    if platform.system() == "Darwin":
        ignored["actual_local_asr_recognizes_known_speech"] = 1
    inventory = Counter({"fixture": 1}) + ignored
    output = "test fixture ... ok\n" + "".join(f"test {name} ... ignored\n" for name in ignored)
    check_tests(inventory, ignored, output)
    for selected, excluded, observed in [
        (Counter(), ignored, output),
        (inventory, ignored + Counter({"undeclared": 1}), output),
        (inventory, ignored, ""),
        (inventory, ignored, output.replace("... ok", "... FAILED")),
    ]:
        try:
            check_tests(selected, excluded, observed)
        except GateError:
            pass
        else:
            raise GateError("FAIL: accounting mutation incorrectly passed")
    print("PASS harness admission: success, deliberate exit 7, timeout, empty/ignored/lost/failed cases")


def main():
    """Dispatch one documented gate mode and return nonzero on any failed observation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["self-test", "headless", "tests", "gui", "smoke"])
    parser.add_argument("--binary", default="target/release/omni")
    parser.add_argument("--evidence-dir", type=Path, default=Path("temp/ci-evidence"))
    parser.add_argument("--timeout-seconds", type=float, default=1800)
    arguments = parser.parse_args()
    if arguments.timeout_seconds <= 0:
        parser.error("timeout must be positive")
    evidence = arguments.evidence_dir.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    timeout = arguments.timeout_seconds
    if arguments.mode == "self-test":
        self_test(evidence)
    elif arguments.mode == "smoke":
        smoke(arguments.binary, evidence, min(timeout, 60))
    elif arguments.mode == "tests":
        test_workspace(evidence, timeout)
    else:
        run(["git", "rev-parse", "HEAD"], evidence, "head", 10)
        for tool in ["ffmpeg", "ffprobe"]:
            run([tool, "-version"], evidence, tool, 10)
        if arguments.mode == "headless":
            run(["cargo", "fmt", "--all", "--", "--check"], evidence, "fmt", timeout)
            test_workspace(evidence, timeout)
            run(["cargo", "clippy", "--workspace", "--all-targets", "--no-default-features", "--locked", "--", "-D", "warnings"], evidence, "clippy", timeout)
            run(["cargo", "build", "--workspace", "--release", "--no-default-features", "--locked"], evidence, "release", timeout)
            target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
            smoke(target / "release/omni", evidence, min(timeout, 60))
        else:
            if platform.system() != "Darwin" or platform.machine() != "arm64":
                raise GateError("FAIL: GUI artifact requires a native macOS ARM64 runner")
            run(["cargo", "clippy", "--workspace", "--all-targets", "--features", "gui", "--locked", "--", "-D", "warnings"], evidence, "gui-clippy", timeout)
            gui_evidence = evidence / "gui-tests"
            gui_evidence.mkdir(exist_ok=True)
            test_workspace(gui_evidence, timeout, gui=True)
            run(["sh", "scripts/bundle-macos.sh", "--ci-unsigned"], evidence, "bundle", timeout)
            smoke("dist/Omni Knowledge.app/Contents/MacOS/omni", evidence, min(timeout, 60))
    print(f"PASS {arguments.mode}")


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, signal.default_int_handler)
    try:
        main()
    except KeyboardInterrupt:
        print("INTERRUPTED: command group stopped", file=sys.stderr)
        sys.exit(130)
    except (GateError, OSError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
