#!/usr/bin/env python3
"""Stage a headless product directory from the Cargo workspace and the existing notice collector.

The binary set is the workspace bin targets whose required features are satisfied by
``--no-default-features``. Vendor executables are omitted. No filename is added unless
Cargo metadata names that bin. ``parser-worker`` is installed beside ``omni`` only when
the parent lookup and the knowledge-processing target both name it. ``omni-fsevents``
is installed beside ``omni`` only for an apple-darwin archive, under the knowledge-local
lookup basename. A Linux copy of that target is not FSEvents proof and is not staged.
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
PARSER_PACKAGE = "knowledge-processing"
PARSER_SOURCE = "parser-worker.rs"
PARSER_LOOKUP = Path("crates/knowledge-processing/src/worker.rs")
PARSER_README = Path("crates/knowledge-processing/README.md")
PARSER_NEEDLE = 'join("parser-worker")'
PARSER_README_TEXT = "else `parser-worker` beside the current executable"
PARSER_BASENAME = "parser-worker"
FSEVENTS_PACKAGE = "knowledge-local"
FSEVENTS_BASENAME = "omni-fsevents"
FSEVENTS_LOOKUP = Path("crates/knowledge-local/src/fsevents.rs")
FSEVENTS_NEEDLE = 'join("omni-fsevents")'
# Cargo artifact basename -> filename beside the product executable.
# The verified maps are identities. A rename stays explicit; nothing is guessed.
PARSER_INSTALL = {"parser-worker": "parser-worker"}
FSEVENTS_INSTALL = {"omni-fsevents": "omni-fsevents"}


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


def apple_target(target: str) -> bool:
    """Return whether this Cargo triple is a macOS archive."""
    return target.endswith("-apple-darwin")


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


def normal_bins(metadata: dict, package_name: str) -> list[dict]:
    """Return normal bin targets for one workspace member. Test and example targets are not helpers."""
    members = set(metadata["workspace_members"])
    found = []
    for package in metadata["packages"]:
        if package["name"] != package_name or package["id"] not in members:
            continue
        for target in package["targets"]:
            if target.get("kind") != ["bin"] or target.get("required-features"):
                continue
            found.append(target)
    return found


def parser_lookup_state(root: Path) -> str:
    """Return absent, declared, or partial for the processing parent lookup."""
    worker_path = root / PARSER_LOOKUP
    readme_path = root / PARSER_README
    worker_hit = worker_path.is_file() and PARSER_NEEDLE in worker_path.read_text(encoding="utf-8")
    readme_hit = readme_path.is_file() and PARSER_README_TEXT in readme_path.read_text(encoding="utf-8")
    if worker_hit and readme_hit:
        return "declared"
    if worker_hit or readme_hit:
        return "partial"
    return "absent"


def fsevents_lookup_declared(root: Path) -> bool:
    """Return whether knowledge-local joins the sibling basename."""
    path = root / FSEVENTS_LOOKUP
    return path.is_file() and FSEVENTS_NEEDLE in path.read_text(encoding="utf-8")


def require_fsevents_lookup(root: Path) -> None:
    """Refuse to guess an FSEvents filename the parent does not join."""
    if not fsevents_lookup_declared(root):
        raise ValueError("parent lookup basename is not omni-fsevents; packaging will not guess")


def require_parser_target(metadata: dict, lookup_state: str) -> dict | None:
    """Return the parser bin when the parent lookup declares it. Do not invent one."""
    found = normal_bins(metadata, PARSER_PACKAGE)
    if lookup_state == "partial":
        raise ValueError("parser-worker parent lookup is partial; packaging will not guess")
    if lookup_state == "absent":
        if found:
            names = ", ".join(target["name"] for target in found)
            raise ValueError(f"knowledge-processing bin without the parent parser-worker lookup: {names}")
        return None
    if len(found) != 1:
        raise ValueError("knowledge-processing does not expose exactly one normal bin")
    target = found[0]
    source = Path(target["src_path"])
    if "tests" in source.parts:
        raise ValueError("refusing a test binary as the parser helper")
    if target["name"] not in PARSER_INSTALL or source.name != PARSER_SOURCE:
        raise ValueError(
            f"cargo helper {target['name']} from {source.name} is not the verified parser-worker map"
        )
    return target


def require_fsevents_target(metadata: dict) -> dict:
    """Return the one knowledge-local bin. A missing or ambiguous target is a hard failure."""
    found = normal_bins(metadata, FSEVENTS_PACKAGE)
    if len(found) != 1:
        raise ValueError("knowledge-local does not expose exactly one normal bin")
    target = found[0]
    source = Path(target["src_path"])
    if "tests" in source.parts:
        raise ValueError("refusing a test binary as omni-fsevents")
    if (target["name"] not in FSEVENTS_INSTALL or source.name != "main.rs"
            or source.parent.name != FSEVENTS_BASENAME):
        raise ValueError(
            f"cargo helper {target['name']} from {source} is not the verified omni-fsevents map"
        )
    return target


def plan_product_bins(selected: list[str], target: str, parser_install: dict[str, str] | None,
                      fsevents_install: dict[str, str]) -> tuple[list[dict], list[dict]]:
    """Map Cargo product bins to installed filenames.

    ``omni`` keeps its Cargo name. A helper is installed only under an explicit map entry.
    ``omni-fsevents`` is omitted from every non-apple archive after Cargo has named it.
    Any other bin fails closed instead of receiving a guessed name.
    """
    if "omni" not in selected:
        raise ValueError("headless graph did not select the omni binary")
    if not fsevents_install:
        raise ValueError("omni-fsevents install map is empty")
    apple = apple_target(target)
    rows, omitted = [], []
    seen, saw_fsevents, saw_parser = set(), False, False
    for cargo_name in selected:
        if cargo_name == "omni":
            installed, role = cargo_name, "product"
        elif parser_install and cargo_name in parser_install:
            installed = parser_install[cargo_name]
            if installed != PARSER_BASENAME:
                raise ValueError("helper install name is not the parent lookup basename")
            role = "parser-helper"
            saw_parser = True
        elif cargo_name in fsevents_install:
            saw_fsevents = True
            installed = fsevents_install[cargo_name]
            if installed != FSEVENTS_BASENAME:
                raise ValueError("fsevents install name is not the parent lookup basename")
            if not apple:
                omitted.append({
                    "cargo_name": cargo_name,
                    "installed_name": installed,
                    "reason": "not an apple-darwin archive; a Linux binary is not FSEvents proof",
                })
                continue
            role = "fsevents-helper"
        else:
            raise ValueError(f"unmapped product bin; refusing a guessed install name: {cargo_name}")
        if installed in seen:
            raise ValueError(f"install name collision: {installed}")
        seen.add(installed)
        rows.append({
            "cargo_name": cargo_name,
            "installed_name": installed,
            "name": installed,
            "role": role,
        })
    if not saw_fsevents:
        raise ValueError("cargo metadata is missing the omni-fsevents target")
    if parser_install and not saw_parser:
        raise ValueError("headless graph did not select exactly one parser helper")
    if apple and not any(row["role"] == "fsevents-helper" for row in rows):
        raise ValueError("apple-darwin archive did not install omni-fsevents")
    if not apple and any(row["role"] == "fsevents-helper" for row in rows):
        raise ValueError("non-apple archive installed omni-fsevents")
    return rows, omitted


def refuse_cloned_product(rows: list[dict]) -> None:
    """Reject a helper whose bytes are a second copy of omni."""
    product = next(row["sha256"] for row in rows if row["role"] == "product")
    for row in rows:
        if row["role"] != "product" and row["sha256"] == product:
            raise ValueError(f"{row['role']} bytes match omni; refusing a second copy of the product binary")


def registry_closure(metadata: dict, package_name: str) -> list[tuple[str, str]]:
    """Return registry crates on one package's normal and build edges, excluding dev edges."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    package_ids = [
        package["id"] for package in metadata["packages"]
        if package["name"] == package_name and package["id"] in members
    ]
    if len(package_ids) != 1 or package_ids[0] not in nodes:
        raise ValueError(f"{package_name} is missing from the locked resolve")
    pending, seen = [package_ids[0]], set()
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            if any(kind.get("kind") in {None, "build"} for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])
    found = []
    for package_id in seen:
        if package_id == package_ids[0]:
            continue
        package = packages[package_id]
        if (package.get("source") or "").startswith("registry+"):
            found.append((package["name"], package["version"]))
    return sorted(set(found))


def missing_registry_closure(metadata: dict, package_name: str, inventory: dict) -> list[str]:
    """Name registry crates of one package that are absent from the staged notice inventory."""
    present = {(entry["name"], entry["version"]) for entry in inventory["packages"]}
    return [
        f"{name}@{version}" for name, version in registry_closure(metadata, package_name)
        if (name, version) not in present
    ]


def notice_packages(rows: list[dict]) -> list[str]:
    """Name packages whose normal closure must appear in the headless notice inventory."""
    names = [PARSER_PACKAGE]
    if any(row["role"] == "fsevents-helper" for row in rows):
        names.append(FSEVENTS_PACKAGE)
    return names


def helper_fields(target: str, parser_target: dict | None, fsevents_target: dict) -> dict:
    """Describe the parser and FSEvents decisions without reading built bytes."""
    if parser_target is None:
        parser_field = {
            "status": "not-selected",
            "reason": "knowledge-processing exposes no normal bin and the parent lookup does not join parser-worker",
        }
    else:
        parser_field = {
            "status": "installed",
            "cargo_name": parser_target["name"],
            "installed_name": PARSER_INSTALL[parser_target["name"]],
            "source": PARSER_SOURCE,
            "lookup": PARSER_BASENAME,
        }
    if apple_target(target):
        fsevents_field = {
            "status": "installed",
            "cargo_name": fsevents_target["name"],
            "installed_name": FSEVENTS_INSTALL[fsevents_target["name"]],
            "source": "src/bin/omni-fsevents/main.rs",
            "lookup": FSEVENTS_BASENAME,
        }
    else:
        fsevents_field = {
            "status": "omitted",
            "cargo_name": fsevents_target["name"],
            "lookup": FSEVENTS_BASENAME,
            "reason": "not an apple-darwin archive; a Linux binary is not FSEvents proof",
        }
    return {"parser_helper": parser_field, "fsevents_helper": fsevents_field}


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


def selection_plan(target: str, metadata: dict) -> tuple[list[str], list[dict], dict | None, dict, list[dict], list[dict]]:
    """Resolve Cargo bins, parent lookups, and the install plan for one triple."""
    require_fsevents_lookup(Path("."))
    lookup_state = parser_lookup_state(Path("."))
    bins, omitted_vendor = selected_bins(metadata)
    parser_target = require_parser_target(metadata, lookup_state)
    fsevents_target = require_fsevents_target(metadata)
    parser_install = None if parser_target is None else PARSER_INSTALL
    planned, omitted_platform = plan_product_bins(bins, target, parser_install, FSEVENTS_INSTALL)
    if parser_target is not None and parser_target["name"] not in {
        row["cargo_name"] for row in planned if row["role"] == "parser-helper"
    }:
        raise ValueError("selected helper does not match the knowledge-processing bin")
    if fsevents_target["name"] not in FSEVENTS_INSTALL:
        raise ValueError("selected helper does not match the knowledge-local bin")
    return bins, omitted_vendor, parser_target, fsevents_target, planned, omitted_platform


def stage_archive(target: str, stage: Path) -> None:
    """Copy built product binaries, the project license, and collector output into a new directory."""
    if stage.exists() or stage.is_symlink():
        raise ValueError("stage must be a new directory")
    metadata = cargo_metadata(target)
    _bins, omitted_vendor, parser_target, fsevents_target, planned, omitted_platform = selection_plan(
        target, metadata)
    built = release_dir(metadata, target)
    create_stage_directory(stage)
    binary_rows = []
    for row in planned:
        source = built / row["cargo_name"]
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"selected binary is not a regular release file: {row['cargo_name']}")
        destination = stage / row["installed_name"]
        if destination.exists() or destination.is_symlink():
            raise ValueError(f"install path already exists: {row['installed_name']}")
        shutil.copy2(source, destination)
        data = destination.read_bytes()
        binary_rows.append({
            "name": row["installed_name"],
            "cargo_name": row["cargo_name"],
            "installed_name": row["installed_name"],
            "role": row["role"],
            "bytes": len(data),
            "sha256": digest(data),
        })
    refuse_cloned_product(binary_rows)
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
    missing = []
    for package_name in notice_packages(planned):
        missing.extend(missing_registry_closure(metadata, package_name, inventory))
    if missing:
        raise ValueError("helper normal closure is missing from the notice inventory: " + ", ".join(missing[:8]))
    gaps = classify_gaps(inventory)
    write_json(stage / "notice-gaps.json", {
        "rule": "Dedicated notice text is collected unchanged. A source archive or fallback is evidence, not a new license.",
        "named_upstream_gaps": gaps,
    })
    fields = helper_fields(target, parser_target, fsevents_target)
    lock_bytes = Path("Cargo.lock").read_bytes()
    write_json(stage / "manifest.json", {
        "kind": "headless-product-archive",
        "target": target,
        "features": [],
        "selection": "workspace bin targets with no required features, excluding vendor executables; helpers installed only under a parent lookup basename",
        "parser_helper": fields["parser_helper"],
        "fsevents_helper": fields["fsevents_helper"],
        "bins": binary_rows,
        "omitted_vendor_bins": omitted_vendor,
        "omitted_platform_helpers": omitted_platform,
        "license": {"path": "LICENSE", "bytes": len(license_bytes), "sha256": digest(license_bytes)},
        "lockfile_sha256": digest(lock_bytes),
        "notice_inventory": "ThirdParty/inventory.json",
    })


def verify_archive(target: str, stage: Path) -> None:
    """Recompute the staged closure and refuse a mismatched binary, lock, helper, or hidden notice gap."""
    metadata = cargo_metadata(target)
    _bins, _omitted_vendor, parser_target, fsevents_target, expected, omitted_platform = selection_plan(
        target, metadata)
    manifest = json.loads((stage / "manifest.json").read_text(encoding="utf-8"))
    if manifest["target"] != target or manifest["features"] != []:
        raise ValueError("manifest target does not match the requested headless archive")
    wanted = {
        (row["cargo_name"], row["installed_name"], row["role"], row["name"])
        for row in expected
    }
    recorded_bins = set()
    for row in manifest["bins"]:
        path = stage / row["name"]
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"staged binary is not a regular file: {row['name']}")
        if row.get("installed_name") != row["name"]:
            raise ValueError(f"installed name does not match the staged filename: {row['name']}")
        if row.get("role") == "product" and row["name"] != row.get("cargo_name"):
            raise ValueError(f"product install name drifted: {row['name']}")
        data = path.read_bytes()
        if digest(data) != row["sha256"] or len(data) != row["bytes"]:
            raise ValueError(f"staged binary does not match the manifest: {row['name']}")
        recorded_bins.add((row.get("cargo_name"), row.get("installed_name"), row.get("role"), row["name"]))
    if recorded_bins != wanted:
        raise ValueError("staged bin map does not match the Cargo selection")
    refuse_cloned_product(manifest["bins"])
    fields = helper_fields(target, parser_target, fsevents_target)
    if manifest.get("parser_helper") != fields["parser_helper"]:
        raise ValueError("manifest parser helper does not match the Cargo target and parent lookup")
    if manifest.get("fsevents_helper") != fields["fsevents_helper"]:
        raise ValueError("manifest FSEvents helper does not match the Cargo target and parent lookup")
    if manifest.get("omitted_platform_helpers") != omitted_platform:
        raise ValueError("manifest platform omissions do not match the Cargo selection")
    names = {row["name"] for row in manifest["bins"]}
    if "omni" not in names:
        raise ValueError("manifest does not contain omni")
    if (stage / "omni").is_symlink():
        raise ValueError("staged omni is not a regular file")
    if apple_target(target):
        if FSEVENTS_BASENAME not in names or not (stage / FSEVENTS_BASENAME).is_file():
            raise ValueError("apple-darwin archive does not install omni-fsevents beside omni")
        if (stage / "omni").parent != (stage / FSEVENTS_BASENAME).parent:
            raise ValueError("omni-fsevents is not beside omni")
    elif (stage / FSEVENTS_BASENAME).exists() or (stage / FSEVENTS_BASENAME).is_symlink():
        raise ValueError("non-apple archive contains omni-fsevents; that file is not FSEvents proof")
    if parser_target is None:
        if (stage / PARSER_BASENAME).exists() or (stage / PARSER_BASENAME).is_symlink():
            raise ValueError("archive contains parser-worker but Cargo did not select it")
    elif PARSER_BASENAME not in names:
        raise ValueError("manifest does not install parser-worker beside omni")
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
    missing = []
    for package_name in notice_packages(expected):
        missing.extend(missing_registry_closure(metadata, package_name, inventory))
    if missing:
        raise ValueError("helper normal closure is missing from the notice inventory: " + ", ".join(missing[:8]))
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
