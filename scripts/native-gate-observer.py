#!/usr/bin/env python3
"""Capture bounded native process facts during the unchanged macOS workspace gate.

Snapshots are NOT ownership, signalling authority, RSS acceptance or settlement.
No signals, command arguments, environment, source contents or keys are read.
The real gate remains foreground-owned even if diagnostic IO fails.
"""
import json
from pathlib import Path
import subprocess
import sys
import time


def rows_from_ps(text):
    rows = {}
    for line in text.splitlines()[:4096]:
        fields = line.split(None, 6)
        if len(fields) != 7:
            continue
        try:
            pid, ppid, pgid = map(int, fields[:3])
            rss = int(fields[5])
        except ValueError:
            continue
        rows[pid] = {"pid": pid, "ppid": ppid, "pgid": pgid,
                     "state": fields[3], "elapsed": fields[4],
                     "native_ps_rss": rss, "native_comm": fields[6]}
    return rows


def main():
    evidence = Path("temp/ci-evidence")
    evidence.mkdir(parents=True, exist_ok=True)
    tracked = {}
    child = subprocess.Popen([sys.executable, "scripts/gate.py", "headless"])
    try:
        with (evidence / "native-process-observations.jsonl").open("w") as log:
            while child.poll() is None:
                result = subprocess.run(
                    ["ps", "-axo", "pid=,ppid=,pgid=,state=,etime=,rss=,comm="],
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                )
                rows = rows_from_ps(result.stdout)
                selected = {child.pid}
                for pid, comm in tracked.items():
                    if pid in rows and rows[pid]["native_comm"] == comm:
                        selected.add(pid)
                while True:
                    more = {pid for pid, row in rows.items() if row["ppid"] in selected}
                    if more <= selected:
                        break
                    selected |= more
                sample = [rows[pid] for pid in sorted(selected) if pid in rows]
                tracked = {row["pid"]: row["native_comm"] for row in sample}
                log.write(json.dumps({"time": time.time(), "gate_pid": child.pid,
                                      "ps_exit": result.returncode, "rows": sample}) + "\n")
                log.flush()
                time.sleep(5)
            log.write(json.dumps({"time": time.time(), "gate_exit": child.returncode,
                                  "all_product_work_settled": "NOT_INFERRED"}) + "\n")
    finally:
        # Never discard the sole actual gate owner after a diagnostic failure.
        code = child.wait()
    return code


if __name__ == "__main__":
    raise SystemExit(main())
