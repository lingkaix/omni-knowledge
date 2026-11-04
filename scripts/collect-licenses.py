#!/usr/bin/env python3
"""Collect an offline, target-specific dependency notice snapshot for a staged artifact.

This development-time tool uses Python's standard library and needs no Python runtime in the app.
It preserves discovered upstream text and explicitly reports missing or partial notice evidence.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
from urllib.parse import quote, urlsplit, urlunsplit

TARGET = "aarch64-apple-darwin"
NOTICE_NAME = re.compile(r"^(?:LICENSE|LICENCE|NOTICE|COPYRIGHT|COPYING|UNLICENSE)(?:[._-]|$)", re.I)
NOTICE_WORDS = re.compile(r"copyright|licen[cs]e|SPDX|all rights reserved|permission is hereby granted", re.I)
EXCLUDED = {".git", ".cargo", "target", "credentials", "credentials.toml", ".env"}
MAX_TEXT_BYTES = 2 * 1024 * 1024
MAX_HEADER_BYTES = 16 * 1024
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024


def digest(data: bytes) -> str:
    """Return a stable content identifier without altering upstream bytes."""
    return hashlib.sha256(data).hexdigest()


def safe_url(value: str | None) -> str | None:
    """Remove URL credentials, queries, and fragments from public provenance records."""
    if not value:
        return None
    parsed = urlsplit(value)
    if parsed.scheme not in {"https", "http"} or not parsed.hostname:
        return None
    host = parsed.hostname
    if parsed.port:
        host += f":{parsed.port}"
    return urlunsplit((parsed.scheme, host, parsed.path, "", ""))


def cargo_metadata(target: str, gui: bool) -> dict:
    """Read the requested locked target graph offline without building or changing resolution."""
    command = ["cargo", "metadata", "--offline", "--locked", "--filter-platform", target,
               "--format-version", "1"]
    command += ["--features", "gui"] if gui else ["--no-default-features"]
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, timeout=60)
    return json.loads(result.stdout)


def reachable_packages(metadata: dict) -> list[dict]:
    """Traverse reachable normal and build edges, excluding the root and all dev-only edges."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    pending, seen = [root], set()
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            if any(kind["kind"] in {None, "build"} for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])
    return sorted((packages[item] for item in seen if item != root),
                  key=lambda package: (package["name"], package["version"], package["id"]))


def locked_checksums(metadata: dict) -> dict:
    """Read archive hashes from Cargo's generated lockfile using its simple package string fields."""
    root_id = metadata["resolve"]["root"]
    package = next(package for package in metadata["packages"] if package["id"] == root_id)
    lockfile = Path(package["manifest_path"]).parent / "Cargo.lock"
    if not lockfile.is_file():
        return {}
    text = read_text(lockfile, 8 * 1024 * 1024)
    checksums = {}
    for block in re.split(r"(?m)^\[\[package\]\]\s*$", text)[1:]:
        values = {}
        for key in ["name", "version", "source", "checksum"]:
            match = re.search(rf'(?m)^{key} = "([^"\\\r\n]*)"$', block)
            if match:
                values[key] = match.group(1)
        if all(key in values for key in ["name", "version", "source", "checksum"]):
            if re.fullmatch(r"[0-9a-f]{64}", values["checksum"]):
                checksums[(values["name"], values["version"], values["source"])] = values["checksum"]
    return checksums


def registry_root(package: dict) -> Path | None:
    """Accept only registry cache packages, excluding workspace, local patches, and Git checkouts."""
    if not (package.get("source") or "").startswith("registry+"):
        return None
    root = Path(package["manifest_path"]).parent
    if (any(path.is_symlink() for path in [root, root.parent, root.parent.parent, root.parent.parent.parent])
        or root.parent.parent.name != "src" or root.parent.parent.parent.name != "registry"):
        return None
    return root


def cached_files(root: Path):
    """Yield contained regular files without following symlinks or entering private/build directories."""
    for parent, directories, names in os.walk(root, followlinks=False):
        directories[:] = sorted(name for name in directories
                                if name not in EXCLUDED and not name.startswith(".env")
                                and not (Path(parent) / name).is_symlink())
        for name in sorted(names):
            path = Path(parent) / name
            if name not in EXCLUDED and not name.startswith(".env") and not path.is_symlink() and path.is_file():
                yield path


def read_text(path: Path, limit: int = MAX_TEXT_BYTES) -> str:
    """Read bounded UTF-8 notice text, rejecting symlinks and binary or oversized notice files."""
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError("notice is not a bounded regular file")
    data = path.read_bytes()
    if len(data) > limit or b"\0" in data:
        raise ValueError("notice exceeds its bound or contains binary data")
    return data.decode("utf-8")


def leading_header(path: Path) -> str:
    """Preserve a leading source comment block when it contains licensing or copyright evidence."""
    with path.open("rb") as source:
        text = source.read(MAX_HEADER_BYTES).decode("utf-8", errors="replace")
    lines, block = [], False
    for line in text.splitlines(keepends=True):
        stripped = line.lstrip()
        if block:
            lines.append(line)
            block = "*/" not in line
        elif not stripped.strip() or stripped.startswith(("//", "#", "/*")):
            lines.append(line)
            block = stripped.startswith("/*") and "*/" not in line
        else:
            break
    header = "".join(lines)
    return header if NOTICE_WORDS.search(header) else ""


def vcs_revision(root: Path) -> str | None:
    """Read only the published crate's bounded VCS marker for matching monorepo provenance."""
    try:
        path = root / ".cargo_vcs_info.json"
        revision = json.loads(read_text(path, 16384)).get("git", {}).get("sha1")
        return revision if isinstance(revision, str) and re.fullmatch(r"[0-9a-f]{40}", revision) else None
    except (OSError, ValueError):
        return None


def collect_text(package: dict, root: Path) -> tuple[list[dict], list[dict], list[str]]:
    """Collect exact notice files, or readable README/source-header fallback evidence when files are absent."""
    files = list(cached_files(root))
    candidates = {path for path in files if NOTICE_NAME.match(path.name)}
    warnings, records, fallbacks = [], [], []
    explicit = package.get("license_file")
    if explicit:
        candidate = Path(explicit)
        if not candidate.is_absolute():
            candidate = root / candidate
        try:
            relative = candidate.relative_to(root)
            if ".." in relative.parts or candidate.is_symlink() or candidate not in files:
                raise ValueError("uncontained or nonregular license_file")
            candidates.add(candidate)
        except ValueError:
            warnings.append("explicit license_file was outside the safe cached package files")
    for path in sorted(candidates):
        try:
            text = read_text(path)
            if not text.strip():
                raise ValueError("empty notice")
            records.append({"path": path.relative_to(root).as_posix(), "text": text})
        except (OSError, UnicodeError, ValueError) as error:
            warnings.append(f"notice could not be read: {path.relative_to(root).as_posix()}: {error}")
    if not records:
        warnings.append("no dedicated upstream notice file found; fallback evidence needs review")
        for path in files:
            try:
                if path.name.lower().startswith("readme"):
                    text = read_text(path)
                    if NOTICE_WORDS.search(text):
                        fallbacks.append({"path": path.relative_to(root).as_posix(), "text": text})
                elif path.suffix.lower() in {".rs", ".c", ".h", ".cpp", ".cc", ".s", ".asm", ".py", ".sh"}:
                    text = leading_header(path)
                    if text:
                        fallbacks.append({"path": path.relative_to(root).as_posix(), "text": text})
            except (OSError, UnicodeError, ValueError):
                continue
    return records, fallbacks, warnings


def source_url(package: dict) -> str | None:
    """Return an exact-version public crates.io source URL only for the official registry."""
    source = package.get("source") or ""
    if source in {"registry+https://github.com/rust-lang/crates.io-index", "registry+https://index.crates.io/"}:
        return f"https://crates.io/api/v1/crates/{quote(package['name'], safe='')}/{quote(package['version'], safe='')}/download"
    return None


def source_archive(package: dict, root: Path, output: Path, lock_checksum: str | None) -> dict:
    """Copy exact cached source after hash/confinement checks for MPL obligations or notice gaps."""
    expected = lock_checksum
    if expected is None:
        checksum_path = root / ".cargo-checksum.json"
        expected = json.loads(read_text(checksum_path)).get("package")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError(f"missing published archive checksum for {package['name']}")
    filename = f"{package['name']}-{package['version']}.crate"
    archive = root.parent.parent.parent / "cache" / root.parent.name / filename
    if (any(path.is_symlink() for path in [archive, archive.parent, archive.parent.parent])
        or not archive.is_file() or archive.stat().st_size > MAX_ARCHIVE_BYTES):
        raise ValueError(f"missing or unsafe crate source archive: {filename}")
    data = archive.read_bytes()
    if len(data) > MAX_ARCHIVE_BYTES or digest(data) != expected:
        raise ValueError(f"crate source archive checksum mismatch: {filename}")
    top = f"{package['name']}-{package['version']}"
    size, count = 0, 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as source:
        for member in source:
            path = PurePosixPath(member.name)
            count += 1
            size += member.size
            if (path.is_absolute() or not path.parts or path.parts[0] != top or ".." in path.parts
                or any(part in EXCLUDED or part.startswith(".env") for part in path.parts)
                or not (member.isfile() or member.isdir()) or count > 20000 or size > MAX_ARCHIVE_BYTES):
                raise ValueError(f"crate source archive contains unsafe or excessive entries: {filename}")
    destination = output / "Sources" / filename
    destination.parent.mkdir(exist_ok=True)
    destination.write_bytes(data)
    return {"path": destination.relative_to(output).as_posix(), "sha256": expected, "bytes": len(data)}


def retained_notices(package: dict, root: Path | None, workspace: Path) -> list[dict]:
    """Reuse hash-verified vendor notice evidence only for the same crate, version, and VCS revision."""
    directory = workspace / "vendor/retrieval-kernel/notices"
    manifest = directory / "UPSTREAM.json"
    if root is None or not manifest.is_file():
        return []
    revision = vcs_revision(root)
    records = []
    for entry in json.loads(read_text(manifest)):
        if (entry["crate"] != package["name"] or entry["version"] != package["version"]
                or not revision or entry["revision"] != revision):
            continue
        relative = PurePosixPath(entry["file"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("uncontained retained notice path")
        path = directory / entry["file"]
        text = read_text(path)
        if digest(text.encode("utf-8")) != entry["sha256"]:
            raise ValueError(f"retained notice checksum mismatch: {entry['file']}")
        records.append({"path": "retained-upstream/" + entry["file"], "text": text})
    return records


def collect(metadata: dict, output: Path, target: str, gui: bool) -> dict:
    """Write target/lock-bound notices and MPL source, retaining explicit coverage gaps."""
    output.mkdir(parents=True, exist_ok=False)
    root_id = metadata["resolve"]["root"]
    root_package = next(package for package in metadata["packages"] if package["id"] == root_id)
    lockfile = Path(metadata["workspace_root"]) / "Cargo.lock"
    lock_digest = digest(read_text(lockfile, 8 * 1024 * 1024).encode("utf-8"))
    packages = reachable_packages(metadata)
    checksums = locked_checksums(metadata)
    inventory, texts, snapshots = [], {}, []
    for package in packages:
        root = registry_root(package)
        entry = {"name": package["name"], "version": package["version"],
                 "declared_license": package.get("license"), "repository": safe_url(package.get("repository")),
                 "versioned_source_url": source_url(package), "notices": [], "fallback_evidence": [], "warnings": []}
        records, fallbacks, warnings = [], [], []
        if root:
            records, fallbacks, warnings = collect_text(package, root)
            records.extend(retained_notices(package, root, Path(metadata["workspace_root"])))
        else:
            warnings.append("nonregistry, local-patched, or unsafe source was excluded from collection")
        snapshots.append((package, root, entry, records, fallbacks, warnings))
    shared = collections.defaultdict(list)
    for package, root, entry, records, _, _ in snapshots:
        revision = vcs_revision(root) if root else None
        if revision and entry["repository"] and records:
            shared[(entry["repository"].removesuffix(".git").rstrip("/"), revision)].append((package, records))
    for package, root, entry, records, fallbacks, warnings in snapshots:
        if not records and root and entry["repository"]:
            key = (entry["repository"].removesuffix(".git").rstrip("/"), vcs_revision(root))
            for origin, notices in shared.get(key, []):
                for notice in notices:
                    fallbacks.append({"path": f"same-repository-same-revision/{origin['name']}@{origin['version']}/{notice['path']}", "text": notice["text"]})
        for field, notices in [("notices", records), ("fallback_evidence", fallbacks)]:
            for notice in notices:
                content_hash = digest(notice["text"].encode("utf-8"))
                texts.setdefault(content_hash, notice["text"])
                entry[field].append({"path": notice["path"], "text_sha256": content_hash})
        if not package.get("license"):
            warnings.append("declared license is unspecified; upstream terms require review")
        if not records and not fallbacks:
            warnings.append("no upstream license or copyright text discovered; metadata and exact source URL only")
        mpl = package.get("license") == "MPL-2.0"
        missing_notice = root is not None and not records and not fallbacks
        if mpl or missing_notice:
            if root is None:
                raise ValueError(f"MPL-only dependency source cannot be collected: {package['name']}")
            key = (package["name"], package["version"], package.get("source"))
            entry["source_archive"] = source_archive(package, root, output, checksums.get(key))
            entry["source_archive_reason"] = "mpl_source_obligation" if mpl else "upstream_notice_gap_evidence"
        entry["warnings"] = warnings
        inventory.append(entry)
    audit = {"target": target, "features": ["gui"] if gui else [],
             "root_package": {"name": root_package["name"], "version": root_package["version"]},
             "lockfile_sha256": lock_digest,
             "scope": "conservative reachable normal and build dependencies; excludes root and dev-only edges; not an exact linked-code inventory",
             "coverage_limit": "Discovered upstream text is preserved. Fallback and missing cases need review; this snapshot is not a public-release compliance certification.",
             "package_count": len(inventory), "declared_license_counts": dict(sorted(collections.Counter(entry["declared_license"] or "UNSPECIFIED" for entry in inventory).items())),
             "packages": inventory}
    (output / "inventory.json").write_text(json.dumps(audit, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    with (output / "THIRD_PARTY_NOTICES.txt").open("w", encoding="utf-8", newline="\n") as document:
        document.write("Omni Knowledge third-party dependency notice snapshot\n\n" + audit["scope"] + "\n" + audit["coverage_limit"] + "\n")
        document.write("Original components retain their licenses; declared expressions are recorded verbatim, including legacy slash notation.\n")
        document.write("Sources/ contains exact gzip-tar .crate archives for MPL source obligations and upstream notice gaps; each retains its declared license and original source evidence.\n")
        for entry in inventory:
            document.write(f"\n=== {entry['name']} {entry['version']} ===\nDeclared license: {entry['declared_license'] or 'UNSPECIFIED'}\n")
            if entry["repository"]:
                document.write(f"Repository: {entry['repository']}\n")
            if entry["versioned_source_url"]:
                document.write(f"Exact-version source: {entry['versioned_source_url']}\n")
            for field in ["notices", "fallback_evidence"]:
                for notice in entry[field]:
                    document.write(f"{field}: {notice['path']} -> full text SHA-256 {notice['text_sha256']}\n")
            if entry.get("source_archive"):
                document.write(f"Included unmodified source ({entry['source_archive_reason']}): {entry['source_archive']['path']} (SHA-256 {entry['source_archive']['sha256']})\n")
            for warning in entry["warnings"]:
                document.write(f"Coverage note: {warning}\n")
        for content_hash, text in sorted(texts.items()):
            document.write(f"\n=== Full upstream text SHA-256 {content_hash} ===\n")
            document.write(text)
            document.write("\n=== End upstream text ===\n")
    return {"packages": len(inventory), "unique_upstream_texts": len(texts),
            "dedicated_notice_files": sum(len(entry["notices"]) for entry in inventory),
            "fallback_review_cases": [f"{entry['name']}@{entry['version']}" for entry in inventory if not entry["notices"]],
            "metadata_only_gaps": [f"{entry['name']}@{entry['version']}" for entry in inventory if not entry["notices"] and not entry["fallback_evidence"]],
            "mpl_source_archives": [entry["source_archive"] for entry in inventory if entry.get("source_archive_reason") == "mpl_source_obligation"],
            "source_archives": [entry["source_archive"] for entry in inventory if "source_archive" in entry],
            "artifact_bytes": sum(path.stat().st_size for path in output.rglob("*") if path.is_file())}


def main() -> int:
    """Generate a fresh staged resource directory and print a concise machine-readable audit summary."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new staged ThirdParty directory; must not already exist")
    parser.add_argument("--target", default=TARGET, help="exact Cargo target triple; default preserves the macOS bundle")
    parser.add_argument("--headless", action="store_true", help="collect the no-default-features CLI graph, not GUI dependencies")
    args = parser.parse_args()
    try:
        summary = collect(cargo_metadata(args.target, not args.headless), args.output, args.target, not args.headless)
    except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(f"Dependency notice collection failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
