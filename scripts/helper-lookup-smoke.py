#!/usr/bin/env python3
"""Smoke installed headless helpers through the public file and watch APIs.

The HTML/EPUB harness is not copied into the product archive. It calls
``extract_file_funded`` and ``read_epub_file_funded`` after ``Account::start`` on an
owned directory descriptor. ``omni --help`` is not this check.

On macOS the same command also runs ``LocalMount::watch`` beside the staged
``omni-fsevents`` binary. On any other host that binary must be absent and the
FSEvents harness is not executed. A Linux exit is not macOS evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import stat
import subprocess
import sys
import tempfile


REPO = Path(__file__).resolve().parent.parent
LOOKUP_SOURCE = REPO / "tests/tools/packaging/lookup_harness.rs"
FSEVENTS_SOURCE = REPO / "tests/tools/packaging/fsevents_harness.rs"
PARSER_NAME = "parser-worker"
FSEVENTS_NAME = "omni-fsevents"


def digest(path: Path) -> str:
    """Return the SHA-256 of one regular file."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_stage(stage: Path, system: str) -> None:
    """Require the archive layout for this host. Do not treat a Linux helper as FSEvents proof."""
    omni = stage / "omni"
    parser = stage / PARSER_NAME
    fsevents = stage / FSEVENTS_NAME
    if omni.is_symlink() or not omni.is_file():
        raise ValueError("stage is missing a regular omni file")
    if parser.is_symlink():
        raise ValueError("parser-worker is a symlink")
    if parser.exists() and (not parser.is_file() or digest(parser) == digest(omni) or not os.access(parser, os.X_OK)):
        raise ValueError("staged parser-worker is not a distinct executable")
    if system == "Darwin":
        if fsevents.is_symlink() or not fsevents.is_file() or not os.access(fsevents, os.X_OK):
            raise ValueError("macOS stage is missing a regular executable omni-fsevents beside omni")
        if fsevents.parent != omni.parent or digest(fsevents) == digest(omni):
            raise ValueError("omni-fsevents is not a distinct sibling of omni")
    elif fsevents.exists() or fsevents.is_symlink():
        raise ValueError("non-macOS stage contains omni-fsevents; refusing to treat it as FSEvents proof")
    if (stage / "lookup-harness").exists() or (stage / "fsevents-harness").exists():
        raise ValueError("refusing a stage that already contains a test harness")


def root_cli_boundary() -> dict:
    """Record whether src/main.rs starts the process account. Do not treat CLI health as lookup proof."""
    text = (REPO / "src/main.rs").read_text(encoding="utf-8")
    bound = "Account::start" in text or "establish_process_account" in text
    return {
        "root_cli_account_startup": "bound" if bound else "unbound",
        "boundary": (
            "src/main.rs calls the process account"
            if bound
            else "src/main.rs does not call Account::start or establish_process_account; "
            "omni --help and omni doctor do not resolve a helper"
        ),
    }


def write_project(project: Path, name: str, source: Path, crates: list[str]) -> None:
    """Write one detached offline harness project. It is not a workspace member."""
    destination = project / "src"
    destination.mkdir(parents=True)
    shutil.copy2(source, destination / "main.rs")
    dependencies = [f'{crate} = {{ path = "{REPO / "crates" / crate}" }}' for crate in crates]
    if name == "parser-lookup-harness":
        dependencies.extend([
            'anyhow = "1"',
            'serde_json = "1"',
            'zip = { version = "4", default-features = false, features = ["deflate"] }',
        ])
    else:
        dependencies.append('serde_json = "1"')
    (project / "Cargo.toml").write_text(
        "\n".join([
            "[package]",
            f'name = "{name}"',
            'version = "0.0.0"',
            'edition = "2024"',
            "publish = false",
            "",
            "[workspace]",
            "",
            "[dependencies]",
            *dependencies,
            "",
        ]),
        encoding="utf-8",
    )


def build_harness(work: Path, name: str, source: Path, crates: list[str]) -> Path:
    """Compile one public-API harness outside the workspace, offline, at two jobs."""
    project = work / name
    write_project(project, name, source, crates)
    env = os.environ.copy()
    env["CARGO_NET_OFFLINE"] = "true"
    env["CARGO_BUILD_JOBS"] = "2"
    target_dir = env.get("CARGO_TARGET_DIR") or str(REPO / "target")
    subprocess.run(
        ["cargo", "generate-lockfile", "--manifest-path", str(project / "Cargo.toml"), "--offline"],
        check=True, cwd=project, env=env, timeout=180,
    )
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(project / "Cargo.toml"), "--offline", "--locked",
         "--release", "--jobs", "2", "--target-dir", target_dir],
        check=True, cwd=project, env=env, timeout=1800,
    )
    binary = Path(target_dir) / "release" / name
    if binary.is_symlink() or not binary.is_file():
        raise ValueError(f"{name} build did not produce a regular binary")
    return binary


def launch(binary: Path, run: Path, env: dict) -> dict:
    """Exec one harness and require the process to be reaped."""
    process = subprocess.Popen(
        [str(binary)], cwd=run, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    # The harness is the actual foreground owner of unknown M3/watch work.
    # Its operation has native deadlines; never kill that owner on a wrapper
    # timer and remove its fixture while group/I/O collection remains unknown.
    stdout, stderr = process.communicate()
    if Path(f"/proc/{process.pid}").exists():
        raise ValueError(f"{binary.name} pid {process.pid} is still visible after wait")
    if process.returncode != 0:
        detail = (stderr or stdout or "").strip()
        raise ValueError(f"{binary.name} failed: pid {process.pid}: {detail}")
    report = json.loads(stdout)
    report["harness_pid_reaped"] = process.pid
    return report


def leftover_helpers(run: Path, names: list[str]) -> list[int]:
    """Return live pids whose executable is one of the helpers copied into this run."""
    proc = Path("/proc")
    if not proc.is_dir():
        return []
    wanted = set()
    for name in names:
        path = run / name
        if path.is_file():
            wanted.add(path.resolve())
    if not wanted:
        return []
    leftovers = []
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            if (entry / "exe").resolve() in wanted:
                leftovers.append(int(entry.name))
        except OSError:
            continue
    return leftovers


def run_lookup(stage: Path, work: Path, harness: Path) -> dict:
    """Exec the HTML/EPUB harness. Copy parser-worker only when the archive actually contains it."""
    run = work / "lookup-run"
    run.mkdir(mode=0o700)
    launched = run / "lookup-harness"
    shutil.copy2(harness, launched)
    launched.chmod(launched.stat().st_mode | stat.S_IXUSR)
    names = []
    parser = stage / PARSER_NAME
    if parser.is_file():
        helper = run / PARSER_NAME
        shutil.copy2(parser, helper)
        helper.chmod(helper.stat().st_mode | stat.S_IXUSR)
        if digest(helper) != digest(parser):
            raise ValueError("run helper does not match the staged parser-worker")
        names.append(PARSER_NAME)
    env = os.environ.copy()
    env.pop("CARGO_BIN_EXE_parser_worker", None)
    env.pop("KNOWLEDGE_PROCESSING_PARSER_WORKER", None)
    report = launch(launched, run, env)
    if report.get("html_contains") is not True or report.get("epub_contains") is not True:
        raise ValueError("public file API did not return the owned HTML and EPUB text")
    if report.get("holdings_finished") is not True or report.get("holdings_after") != 0:
        raise ValueError("helper lookup smoke did not finish account holdings")
    system = platform.system()
    if names:
        if system == "Linux":
            pids = report.get("child_pids")
            if not isinstance(pids, list) or not pids or not all(isinstance(pid, int) and pid > 0 for pid in pids):
                raise ValueError("Linux smoke did not observe a real parser-worker pid")
            if report.get("children_reaped") is not True or report.get("child_pid_sample") != "own-children-proc-exe":
                raise ValueError("Linux smoke did not collect the parser-worker child")
        elif report.get("child_pid_sample") != "not-linux":
            raise ValueError("non-Linux parser smoke invented a Linux pid sample")
    else:
        if report.get("lookup_basename") is not None or report.get("child_pids") not in ([],):
            raise ValueError("smoke invented a parser-worker lookup or pid")
        if report.get("child_pid_sample") != "in-process" or report.get("children_reaped") is not True:
            raise ValueError("in-process extraction was reported as a helper process")
    leftovers = leftover_helpers(run, names)
    if leftovers:
        raise ValueError(f"helper child still alive after the harness exited: {leftovers}")
    report["platform"] = system
    return report


def run_fsevents(stage: Path, work: Path, harness: Path) -> dict:
    """Exec the macOS watch harness beside the staged sibling. Do not call this on Linux."""
    if platform.system() != "Darwin":
        raise ValueError("refusing to run the FSEvents harness off macOS")
    run = work / "fsevents-run"
    run.mkdir(mode=0o700)
    helper = run / FSEVENTS_NAME
    shutil.copy2(stage / FSEVENTS_NAME, helper)
    helper.chmod(helper.stat().st_mode | stat.S_IXUSR)
    if digest(helper) != digest(stage / FSEVENTS_NAME):
        raise ValueError("run omni-fsevents does not match the staged helper")
    launched = run / "fsevents-harness"
    shutil.copy2(harness, launched)
    launched.chmod(launched.stat().st_mode | stat.S_IXUSR)
    env = os.environ.copy()
    env.pop("CARGO_BIN_EXE_omni_fsevents", None)
    report = launch(launched, run, env)
    if report.get("event_delivered") is not True or report.get("cursor_observed") is not True:
        raise ValueError("macOS watch did not deliver a native cursor")
    if report.get("close") != "confirmed" or report.get("holdings_settled") is not True:
        raise ValueError("macOS watch did not close and settle its holding")
    if report.get("lookup") != "sibling-omni-fsevents":
        raise ValueError("macOS watch did not use the sibling lookup")
    leftovers = leftover_helpers(run, [FSEVENTS_NAME])
    if leftovers:
        raise ValueError(f"omni-fsevents still alive after close: {leftovers}")
    return report


def main() -> int:
    """Build the harnesses, run the host-appropriate checks, and write the JSON evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", required=True, type=Path, help="unpacked headless archive root")
    parser.add_argument("--evidence-dir", required=True, type=Path, help="directory for the JSON result")
    args = parser.parse_args()
    os.chdir(REPO)
    evidence = args.evidence_dir
    evidence.mkdir(parents=True, exist_ok=True)
    system = platform.system()
    try:
        require_stage(args.stage, system)
        boundary = root_cli_boundary()
        with tempfile.TemporaryDirectory(prefix="omni-m8-lookup-") as temporary:
            work = Path(temporary)
            lookup = build_harness(
                work, "parser-lookup-harness", LOOKUP_SOURCE, ["knowledge-processing", "knowledge-core"])
            report = run_lookup(args.stage, work, lookup)
            if system == "Darwin":
                fsevents = build_harness(
                    work, "fsevents-harness", FSEVENTS_SOURCE, ["knowledge-local", "knowledge-core"])
                native = run_fsevents(args.stage, work, fsevents)
                report["fsevents_smoke"] = "ran"
                report["fsevents"] = native
                (evidence / "fsevents-lookup.json").write_text(
                    json.dumps(native, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            else:
                report["fsevents_smoke"] = "unobserved"
                report["native_mac"] = "unobserved"
        report.update(boundary)
        if report["root_cli_account_startup"] != "unbound":
            report["root_cli_note"] = "account startup appeared in src/main.rs; this smoke still does not call the CLI"
        (evidence / "helper-lookup.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Helper lookup smoke failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"stage": str(args.stage), "evidence": str(evidence / "helper-lookup.json")}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
