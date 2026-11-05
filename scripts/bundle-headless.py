#!/usr/bin/env python3
"""Stage a headless product directory from the Cargo workspace and the existing notice collector.

The binary set is the workspace bin targets whose required features are satisfied by
``--no-default-features``. Vendor executables are omitted: the kernel owner states that
the root product does not call the retrieval-kernel executable, and its seven faces stay
in the headless test gate. No filename is added unless Cargo metadata names that bin.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

NAMED_UPSTREAM_GAPS = (
    "adobe-cmap-parser",
    "htmlescape",
    "pdf-extract",
    "type1-encoding-parser",
)


def digest(data: bytes) -> str:
    """Return the SHA-256 hex digest of exact bytes."""
    return hashlib.sha256(data).hexdigest()


def run_json(command: list[str], timeout: int) -> dict:
    """Run one local command and parse a single JSON document from its stdout."""
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, timeout=timeout)
    return json.loads(result.stdout)


def host_triple() -> str:
    """Read the executing rustc host triple. This does not download a target."""
    result = subprocess.run(["rustc", "-vV"], check=True, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, timeout=30)
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.split(": ", 1)[1].strip()
    raise ValueError("rustc did not report a host triple")


def cargo_metadata(target: str) -> dict:
    """Read the locked headless graph for one target without resolving or building."""
    return run_json([
        "cargo", "metadata", "--offline", "--locked", "--no-default-features",
        "--filter-platform", target, "--format-version", "1",
    ], 120)


def release_dir(metadata: dict, target: str) -> Path:
    """Locate Cargo's release directory for the host or an explicitly different target."""
    base = Path(metadata["target_directory"])
    if target == host_triple():
        return base / "release"
    return base / target / "release"


def selected_bins(metadata: dict) -> tuple[list[str], list[dict]]:
    """Return product bin names and the vendor bins this archive deliberately omits."""
    workspace = Path(metadata["workspace_root"]).resolve()
    members = set(metadata["workspace_members"])
    selected, omitted = [], []
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        manifest = Path(package["manifest_path"]).resolve()
        try:
            vendor = "vendor" in manifest.relative_to(workspace).parts
        except ValueError:
            vendor = False
        for target in package["targets"]:
            if "bin" not in target["kind"]:
                continue
            required = target.get("required-features") or []
            if required:
                continue
            record = {"package": package["name"], "name": target["name"]}
            if vendor:
                omitted.append(record)
            else:
                selected.append(target["name"])
    if len(selected) != len(set(selected)):
        raise ValueError("two product packages selected the same binary name")
    return sorted(selected), omitted


def classify_gaps(inventory: dict) -> list[dict]:
    """Describe named upstream crates from collector evidence without adding license text."""
    packages = {entry["name"]: entry for entry in inventory["packages"]}
    rows = []
    for name in NAMED_UPSTREAM_GAPS:
        entry = packages.get(name)
        if entry is None:
            rows.append({"name": name, "present": False,
                         "resolution": "not in this headless normal/build graph"})
            continue
        notices = entry.get("notices") or []
        fallbacks = entry.get("fallback_evidence") or []
        archive = entry.get("source_archive")
        if notices:
            resolution = "dedicated notice text is in the cached crate"
        elif archive:
            resolution = "exact cached crate source retained; no dedicated notice text invented"
        elif fallbacks:
            resolution = "fallback text only; not a dedicated notice and not a compliance claim"
        else:
            raise ValueError(f"{name} is in the graph with no notice, fallback, or source archive")
        rows.append({
            "name": name,
            "present": True,
            "version": entry["version"],
            "dedicated_notices": len(notices),
            "fallback_evidence": len(fallbacks),
            "source_archive_sha256": None if archive is None else archive["sha256"],
            "resolution": resolution,
        })
    return rows


def write_json(path: Path, value: dict) -> None:
    """Write one UTF-8 JSON document with a trailing newline."""
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def create_stage_directory(stage: Path) -> None:
    """Create a fresh stage and missing parents without reusing an existing entry."""
    if stage.exists() or stage.is_symlink():
        raise ValueError("stage must be a new directory")
    stage.mkdir(parents=True)


def stage_archive(target: str, stage: Path) -> None:
    """Copy built product binaries, the project license, and collector output into a new directory."""
    if stage.exists() or stage.is_symlink():
        raise ValueError("stage must be a new directory")
    metadata = cargo_metadata(target)
    bins, omitted = selected_bins(metadata)
    if "omni" not in bins:
        raise ValueError("headless graph did not select the omni binary")
    built = release_dir(metadata, target)
    create_stage_directory(stage)
    binary_rows = []
    for name in bins:
        source = built / name
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"selected binary is not a regular release file: {name}")
        destination = stage / name
        shutil.copy2(source, destination)
        data = destination.read_bytes()
        binary_rows.append({"name": name, "bytes": len(data), "sha256": digest(data)})
    license_path = Path("LICENSE")
    if license_path.is_symlink() or not license_path.is_file():
        raise ValueError("project LICENSE is not a regular file")
    shutil.copy2(license_path, stage / "LICENSE")
    license_bytes = (stage / "LICENSE").read_bytes()
    third_party = stage / "ThirdParty"
    subprocess.run([
        sys.executable, "-B", "scripts/collect-licenses.py", "--target", target, "--headless",
        str(third_party),
    ], check=True, timeout=180)
    inventory = json.loads((third_party / "inventory.json").read_text(encoding="utf-8"))
    if inventory["target"] != target or inventory["features"] != []:
        raise ValueError("notice inventory does not match the headless target")
    gaps = classify_gaps(inventory)
    write_json(stage / "notice-gaps.json", {
        "rule": "Dedicated notice text is collected unchanged. A source archive or fallback is evidence, not a new license.",
        "named_upstream_gaps": gaps,
    })
    lock_bytes = Path("Cargo.lock").read_bytes()
    write_json(stage / "manifest.json", {
        "kind": "headless-product-archive",
        "target": target,
        "features": [],
        "selection": "workspace bin targets with no required features, excluding vendor executables",
        "bins": binary_rows,
        "omitted_vendor_bins": omitted,
        "license": {"path": "LICENSE", "bytes": len(license_bytes), "sha256": digest(license_bytes)},
        "lockfile_sha256": digest(lock_bytes),
        "notice_inventory": "ThirdParty/inventory.json",
    })


def verify_archive(target: str, stage: Path) -> None:
    """Recompute the staged closure and refuse a mismatched binary, lock, or hidden notice gap."""
    manifest = json.loads((stage / "manifest.json").read_text(encoding="utf-8"))
    if manifest["target"] != target or manifest["features"] != []:
        raise ValueError("manifest target does not match the requested headless archive")
    for row in manifest["bins"]:
        data = (stage / row["name"]).read_bytes()
        if digest(data) != row["sha256"] or len(data) != row["bytes"]:
            raise ValueError(f"staged binary does not match the manifest: {row['name']}")
    if "omni" not in {row["name"] for row in manifest["bins"]}:
        raise ValueError("manifest does not contain omni")
    license_row = manifest["license"]
    if digest((stage / "LICENSE").read_bytes()) != license_row["sha256"]:
        raise ValueError("staged LICENSE does not match the manifest")
    inventory = json.loads((stage / "ThirdParty" / "inventory.json").read_text(encoding="utf-8"))
    if inventory["target"] != target or inventory["features"] != []:
        raise ValueError("unpacked notice inventory does not match the headless target")
    if inventory["lockfile_sha256"] != digest(Path("Cargo.lock").read_bytes()):
        raise ValueError("notice inventory is not bound to this Cargo.lock")
    if not any(entry["name"] == "tantivy" and entry["notices"] for entry in inventory["packages"]):
        raise ValueError("Tantivy notice text is missing from the unpacked inventory")
    gaps = classify_gaps(inventory)
    recorded = json.loads((stage / "notice-gaps.json").read_text(encoding="utf-8"))
    if recorded.get("named_upstream_gaps") != gaps:
        raise ValueError("notice-gaps.json does not match the unpacked inventory")
    for row in gaps:
        if row["present"] and row["dedicated_notices"] == 0 and row["source_archive_sha256"] is None and row["fallback_evidence"] == 0:
            raise ValueError(f"named gap has no retained evidence: {row['name']}")


def main() -> int:
    """Stage a new directory, or verify an unpacked directory, for one headless target."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, help="Cargo target triple of the built binaries")
    parser.add_argument("--stage", required=True, type=Path, help="new stage directory, or the unpacked archive root with --verify")
    parser.add_argument("--verify", action="store_true", help="check an existing stage without copying or collecting")
    args = parser.parse_args()
    os.chdir(Path(__file__).resolve().parent.parent)
    try:
        if args.verify:
            verify_archive(args.target, args.stage)
        else:
            stage_archive(args.target, args.stage)
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"Headless bundle failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"target": args.target, "stage": str(args.stage), "verify": args.verify}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
