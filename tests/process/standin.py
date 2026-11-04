#!/usr/bin/env python3
"""Supply synthetic external tools for Linux-only bundle lifecycle admission.

This fixture never replaces the gate, bundler, or their build delegation.
Every write is under the admission case directory supplied by the test.
Unexpected tool requests fail closed instead of invoking a host compiler.
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(os.environ["PROCESS_CASE"])


def event(kind, **fields):
    """Append one small, atomic fixture observation; propagate I/O errors."""
    row = {"event": kind, "pid": os.getpid(), "time": time.monotonic(), **fields}
    fd = os.open(ROOT / "events.jsonl", os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        os.write(fd, (json.dumps(row) + "\n").encode())
    finally:
        os.close(fd)


def ancestors():
    """Return owned ancestor commands, stopping at the fixture driver."""
    result = []
    pid = os.getpid()
    while pid > 1:
        try:
            directory = Path("/proc") / str(pid)
            fields = (directory / "stat").read_text().rsplit(")", 1)[1].split()
            result.append({"pid": pid, "argv": (directory / "cmdline").read_bytes().decode().split("\0")[:-1]})
            if pid == int(os.environ["PROCESS_DRIVER_PID"]):
                break
            pid = int(fields[1])
        except FileNotFoundError:
            break
    return result


def controlled_effect(tool, chain):
    """Block only a selected nested tool, with a ready descendant and late write.

    The child inherits its session normally; the fixture does not detach it.
    A release after outer termination exposes residual write capability.
    A finite deadline bounds damage even if the test driver disappears.
    """
    mode = os.environ["PROCESS_MODE"]
    event("selected", tool=tool, ancestors=chain, mode=mode)
    if mode == "failure":
        return 37
    if mode == "success":
        return 0
    child = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "worker"])
    deadline = time.monotonic() + 30
    while not (ROOT / "worker-ready").exists():
        if child.poll() is not None or time.monotonic() > deadline:
            return 38
        time.sleep(0.01)
    (ROOT / "ready.tmp").write_text(json.dumps({"tool": tool, "pid": os.getpid(), "child": child.pid, "ancestors": chain}))
    (ROOT / "ready.tmp").replace(ROOT / "ready")
    return child.wait()


def main():
    """Dispatch a stand-in tool request and return its synthetic exit status."""
    tool = Path(sys.argv[0]).name
    args = sys.argv[1:]
    if args == ["worker"]:
        event("worker-ready", pgid=os.getpgrp(), sid=os.getsid(0))
        (ROOT / "worker-ready").touch()
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if (ROOT / "release").exists():
                (ROOT / "delayed-write").write_text("owned descendant wrote after terminal observation\n")
                event("delayed-write")
                return 0
            time.sleep(0.01)
        return 39
    chain = ancestors()
    nested = any(any(Path(arg).name == "bundle-macos.sh" for arg in row["argv"]) for row in chain)
    event("tool", tool=tool, args=args, nested=nested)
    if tool == "prepare-dictionary.py":
        if nested and os.environ["PROCESS_ROUTE"] == "supplier":
            return controlled_effect("supplier", chain)
        return 0
    if tool == "git":
        if args == ["rev-parse", "HEAD"]:
            print(os.environ["PROCESS_SUBJECT_COMMIT"])
        elif args != ["status", "--short"]:
            raise ValueError("unexpected Git prerequisite: " + repr(args))
    elif tool == "uname":
        print("arm64" if "-m" in args else "Darwin")
    elif tool in ("rustc", "ffmpeg", "ffprobe"):
        print(tool + " synthetic admission version")
    elif tool == "cargo":
        if args == ["--version"]:
            print("cargo synthetic admission version")
        elif "metadata" in args:
            root = Path.cwd()
            package = {"id": "fixture 0.0.0", "name": "fixture", "version": "0.0.0", "source": None,
                       "manifest_path": str(root / "Cargo.toml"), "license": "Apache-2.0", "dependencies": []}
            print(json.dumps({"target_directory": os.environ["CARGO_TARGET_DIR"], "workspace_root": str(root),
                              "version": 1, "packages": [package], "workspace_members": [package["id"]],
                              "resolve": {"root": package["id"], "nodes": [
                                  {"id": package["id"], "dependencies": [], "deps": [], "features": ["gui"]}]}}))
        elif "test" in args:
            if "--list" in args:
                if "--ignored" not in args:
                    print("fixture_admission: test")
                print("actual_local_asr_recognizes_known_speech: test")
            else:
                print("running 2 tests\ntest fixture_admission ... ok\n"
                      "test actual_local_asr_recognizes_known_speech ... ignored\n\n"
                      "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out")
        elif "build" in args:
            if nested and os.environ["PROCESS_ROUTE"] == "cargo":
                status = controlled_effect("cargo", chain)
                if status:
                    return status
            target = Path(os.environ["CARGO_TARGET_DIR"]) / "release"
            target.mkdir(parents=True, exist_ok=True)
            for name in ("omni", "omni-knowledge-gui"):
                output = target / name
                output.write_text('#!/usr/bin/env python3\n"""Synthetic CLI prerequisite; not a product artifact."""\nimport json, sys\nprint("synthetic admission help" if "--help" in sys.argv else json.dumps({"ffmpeg": {"available": True}, "ffprobe": {"available": True}}))\n')
                output.chmod(0o755)
        elif "clippy" not in args and "fmt" not in args:
            raise ValueError("unhandled Cargo prerequisite: " + repr(args))
    elif tool == "plutil":
        return 0
    elif tool == "codesign":
        event("signing", args=args)
    else:
        raise ValueError("unexpected external tool: " + tool)
    return 0


if __name__ == "__main__":
    sys.exit(main())
