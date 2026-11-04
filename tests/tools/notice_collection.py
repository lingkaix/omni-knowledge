#!/usr/bin/env python3
"""Characterization tests for the committed license-notice collector CLI.

The subject is the public `scripts/collect-licenses.py`, exported as opaque bytes through `git show` from one selected checkout and bound by the recorded commit and SHA-256 of the exported file.
The default current mode selects this checkout HEAD, or an explicit `--subject-repo`/`--current-ref`, so a run gates the actual checked-out source; `--legacy` selects the pinned historical characterization with committed revisions f80813f775d23138496750f6753f61d341aaf65b (new side) and a4d77f06fda38f197653ef4ec82425e4df6aa8f3 (old side).
Expected values never come from reading the subject implementation; they come from the public owner contract and from fixture bytes declared in this file:

- [product licensing](../../docs/specs/product.md) `#project-license`: third-party components keep their original licenses and provenance.
- [dependency guidance](../../docs/dependencies.md): `Cargo.lock` fixes versions, notices and exact unmodified MPL-2.0 source archives ship with the bundle, and the generated inventory states coverage gaps instead of claiming an audit.
- Committed revision f80813f `scripts/README.md` (collector section) and `docs/changes/2026-11-04-linux-artifact-notices.md`: default Darwin GUI selection is preserved; `--target <triple> --headless OUTPUT` collects the Linux headless graph; `OUTPUT` must be a new directory; the inventory binds target, selected features, root package, and the exact workspace lock digest; retained upstream notice reuse requires matching crate/version/VCS identity and a valid retained hash; a registry notice gap keeps the exact lock-checksummed original source archive and reports the gap honestly; coverage gaps stay explicitly flagged as nonempty classifications rather than exact English sentences.

The external Cargo metadata effect is stood in by a synthetic `cargo` shim that serves a static Cargo-format graph plus a synthetic registry/cache; the real collector CLI, file evidence, hashes, artifact bytes, and deciding assertions remain live.
These are bounded instrument/regression tests: they do not prove the real Linux dependency graph, macOS bundling, or legal compliance.

Run with: `python3 -B tests/tools/notice_collection.py` (standard library only, Python 3.9+).
Current mode resolves the selected ref to one exact commit, records the digest of the exported executed file, and fails on a missing selected ref without fallback; it needs no historical objects, and the legacy-only OLD/NEW parity test is reported as not selected instead of silently skipped.
`--legacy` additionally needs a checkout holding both pinned revisions; set `NOTICE_COLLECTION_SUBJECT_REPO` or pass `--subject-repo` to name one, otherwise the legacy search tries this checkout and the sibling lead checkout.
Fixtures are disposable synthetic workspaces under the ignored `temp/notice-collection-tests/admission/` root; the root is removed after a fully successful run and retained otherwise so failing observations stay inspectable.
"""

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from collections import namedtuple
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ADMISSION_ROOT = REPO_ROOT / "temp" / "notice-collection-tests" / "admission"

SUBJECT_REPO_ENV = "NOTICE_COLLECTION_SUBJECT_REPO"
COLLECTOR_PATH_IN_REPOSITORY = "scripts/collect-licenses.py"

# Legacy mode pins the previously characterized revisions; they are declared prerequisites of that explicit mode, never the default current subject.
LEGACY_NEW_COMMIT = "f80813f775d23138496750f6753f61d341aaf65b"
LEGACY_NEW_SHA256 = "e3bc710d94b36798f2ac3016dec75c31ad521e0386099ec20b92e60310710376"
LEGACY_OLD_COMMIT = "a4d77f06fda38f197653ef4ec82425e4df6aa8f3"
LEGACY_OLD_SHA256 = "b813b4e73d85016a0a477ac7b77c363c2ee40569fdc47fb7d3fb3feb6875eb77"

DARWIN_TARGET = "aarch64-apple-darwin"
LINUX_TARGET = "x86_64-unknown-linux-gnu"
COLLECT_RUN_TIMEOUT_SECONDS = 180

REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
REGISTRY_INDEX_DIR = "index.crates.io-6146d94d89ad2bcd"
ROOT_NAME, ROOT_VERSION = "fixture-root", "1.4.2"
# 40 lowercase hex characters; the retained evidence revision and the VCS identity must match it.
RETAINED_REVISION = "4f1c9d2e8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d"

CRATE_MANIFEST = '[package]\nname = "%s"\nversion = "%s"\nlicense = "%s"\n'
LICENSE_ALPHA = "Synthetic MIT notice text for alpha-core 1.0.0 fixture.\n"
LICENSE_BETA = "Synthetic Apache notice text for beta-build 2.3.1 fixture.\n"
LICENSE_DELTA = "Synthetic ISC notice text for delta-dev 3.1.0 fixture.\n"
LICENSE_EPSILON = "Synthetic Apache notice text for epsilon-gui 0.2.0 fixture.\n"
LICENSE_RETAINED = "Synthetic retained upstream notice text for retained-gem 0.9.0.\n"
# Declared replacement digest material for the wrong-retained-hash negative control.
TAMPERED_RETAINED_BYTES = b"tampered retained evidence bytes\n"

# name -> (version, declared SPDX license, discovered notice text or None).
CRATES = {
    "alpha-core": ("1.0.0", "MIT", LICENSE_ALPHA),
    "beta-build": ("2.3.1", "Apache-2.0", LICENSE_BETA),
    "delta-dev": ("3.1.0", "ISC", LICENSE_DELTA),
    "epsilon-gui": ("0.2.0", "Apache-2.0", LICENSE_EPSILON),
    "gamma-transitive": ("0.5.0", "MIT", None),
    "mpl-lib": ("1.2.3", "MPL-2.0", None),
    "retained-gem": ("0.9.0", "Apache-2.0", None),
}
# Root dependency edges: (crate, kind) where None is a normal edge.
ROOT_EDGES = [
    ("alpha-core", None),
    ("beta-build", "build"),
    ("delta-dev", "dev"),
    ("epsilon-gui", None),
    ("mpl-lib", None),
    ("retained-gem", None),
]
# alpha-core pulls gamma-transitive as a normal transitive dependency.
TRANSITIVE_EDGES = {"alpha-core": [("gamma-transitive", None)]}

# The OLD/NEW parity case needs both pinned historical revisions; it is selected only with --legacy.
LEGACY_ONLY_TESTS = frozenset({"test_default_selection_matches_older_committed_source"})

SUBJECT_CACHE = {}

FAKE_CARGO_SOURCE = """#!/usr/bin/env python3
# Synthetic stand-in for the external Cargo metadata effect (test instrument).
# Records each invocation and prints a static Cargo-format metadata document.
import json
import os
import sys

RECORD = os.environ["FAKE_CARGO_RECORD"]
DEFAULT = os.environ["FAKE_CARGO_METADATA"]

with open(RECORD, "a", encoding="utf-8") as handle:
    handle.write(json.dumps(sys.argv[1:]) + "\\n")

command = sys.argv[1] if len(sys.argv) > 1 else ""
if command != "metadata":
    sys.stderr.write("fake-cargo: unexpected command: %s\\n" % command)
    raise SystemExit(70)

selected = DEFAULT
platform = None
arguments = sys.argv[2:]
for index, value in enumerate(arguments):
    if value == "--filter-platform" and index + 2 < len(arguments):
        platform = arguments[index + 1]
    if value.startswith("--filter-platform="):
        platform = value.split("=", 1)[1]
if platform:
    candidate = os.path.join(DEFAULT, "platform-%s.json" % platform)
    if os.path.isfile(candidate):
        selected = candidate

with open(selected, encoding="utf-8") as handle:
    document = json.load(handle)
json.dump(document, sys.stdout)
"""


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


Subject = namedtuple("Subject", ["repository", "ref", "commit", "sha256", "path"])

SELECTED = {"current": None, "legacy_repo": None}


def select_current_subject(subject):
    """Record the subject that the selected test set executes."""
    SELECTED["current"] = subject


def current_subject():
    """Return the selected current subject; raise when nothing was selected."""
    subject = SELECTED["current"]
    if subject is None:
        raise RuntimeError("no current subject was selected; run through main() or pass --subject-repo/--current-ref")
    return subject


def git_check_output(repository, arguments, guidance):
    """Run one read-only git query in the subject repository and return its stdout bytes."""
    probe = subprocess.run(["git", "-C", str(repository), *arguments], capture_output=True, timeout=30)
    if probe.returncode != 0:
        raise RuntimeError("%s\n%s" % (guidance, probe.stderr.decode("utf-8", "replace").strip()))
    return probe.stdout


def write_subject_export(repository, ref, commit, data):
    """Write the opaque subject bytes and return the identity bound to the written, executed file."""
    ADMISSION_ROOT.mkdir(parents=True, exist_ok=True)
    subject = ADMISSION_ROOT / ("collect-%s.py" % commit[:8])
    subject.write_bytes(data)
    digest = sha256_bytes(subject.read_bytes())
    return Subject(repository=str(repository), ref=ref, commit=commit, sha256=digest, path=subject)


def resolve_current_subject(repository, ref):
    """Resolve one exact subject commit from the selected checkout and export its opaque bytes.

    The selected ref must resolve in the selected repository; there is no fallback repository and no pinned substitute.
    """
    probe = subprocess.run(["git", "-C", str(repository), "rev-parse", "--git-dir"], capture_output=True, timeout=30)
    if probe.returncode != 0:
        raise RuntimeError("%s is not a git checkout; pass --subject-repo naming a checkout that holds the subject" % repository)
    shown = git_check_output(
        repository, ["rev-parse", "--verify", "%s^{commit}" % ref],
        "selected ref %s does not resolve in %s; there is no fallback to another repository or pinned commit" % (ref, repository),
    )
    commit = shown.decode("utf-8").strip()
    data = git_check_output(
        repository, ["show", "%s:%s" % (commit, COLLECTOR_PATH_IN_REPOSITORY)],
        "%s does not contain %s at %s" % (repository, COLLECTOR_PATH_IN_REPOSITORY, commit),
    )
    return write_subject_export(repository, ref, commit, data)


def legacy_search_repositories(explicit):
    """Yield local checkouts that may contain the pinned legacy revisions."""
    if explicit:
        yield Path(explicit)
    yield REPO_ROOT
    yield REPO_ROOT.parents[1] / "lead" / "omni-knowledge"


def export_pinned_subject(commit, expected_sha256, explicit_repository):
    """Export a pinned legacy revision as opaque bytes and bind its declared identity.

    Searches the candidate checkouts and refuses any bytes that differ from the declared digest.
    """
    cached = SUBJECT_CACHE.get(commit)
    if cached is not None:
        return cached
    attempts = []
    for repository in legacy_search_repositories(explicit_repository):
        if not (repository / ".git").exists():
            attempts.append("%s (no checkout)" % repository)
            continue
        probe = subprocess.run(
            ["git", "-C", str(repository), "cat-file", "-e", "%s:%s" % (commit, COLLECTOR_PATH_IN_REPOSITORY)],
            capture_output=True, timeout=30,
        )
        if probe.returncode != 0:
            attempts.append("%s (commit or path absent)" % repository)
            continue
        data = git_check_output(repository, ["show", "%s:%s" % (commit, COLLECTOR_PATH_IN_REPOSITORY)], "%s lookup failed" % repository)
        actual = sha256_bytes(data)
        if actual != expected_sha256:
            raise RuntimeError(
                "exported %s from %s has SHA-256 %s, expected %s"
                % (commit, repository, actual, expected_sha256)
            )
        subject = write_subject_export(repository, commit, commit, data)
        SUBJECT_CACHE[commit] = subject
        return subject
    raise RuntimeError(
        "no local checkout with %s:%s was found; set %s or pass --subject-repo naming one that contains it. Tried: %s"
        % (commit, COLLECTOR_PATH_IN_REPOSITORY, SUBJECT_REPO_ENV, "; ".join(attempts))
    )


def legacy_subjects():
    """Export both pinned legacy revisions for the explicit OLD/NEW parity characterization."""
    repository = SELECTED["legacy_repo"]
    return (
        export_pinned_subject(LEGACY_NEW_COMMIT, LEGACY_NEW_SHA256, repository),
        export_pinned_subject(LEGACY_OLD_COMMIT, LEGACY_OLD_SHA256, repository),
    )


def crate_archive_bytes(name, version, license_id):
    """Deterministic gzip-tar .crate bytes carrying the declared crate manifest."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w") as archive:
        payload = (CRATE_MANIFEST % (name, version, license_id)).encode("utf-8")
        info = tarfile.TarInfo(name="%s-%s/Cargo.toml" % (name, version))
        info.size = len(payload)
        archive.addfile(info, io.BytesIO(payload))
    return gzip.compress(buffer.getvalue(), mtime=0)


class Fixture:
    """A synthetic workspace, registry/cache, and cargo stand-in for one test."""

    def __init__(self, root):
        self.root = root
        self.workspace = root / "workspace"
        self.cargo_home = root / "home" / ".cargo"
        self.fake_bin = root / "fake-bin"
        self.archive_sha256 = {}
        self.archive_bytes = {}
        self.lock_sha256 = None
        self.retained_notice_sha256 = None
        self._build()

    def crate_id(self, name):
        if name == ROOT_NAME:
            return "%s %s (path+file://%s)" % (ROOT_NAME, ROOT_VERSION, self.workspace)
        version = CRATES[name][0]
        return "%s %s (%s)" % (name, version, REGISTRY_SOURCE)

    # -- construction ------------------------------------------------------

    def _build(self):
        for directory in (self.workspace / "tools", self.fake_bin, self.cargo_home):
            directory.mkdir(parents=True, exist_ok=True)
        (self.fake_bin / "cargo").write_text(FAKE_CARGO_SOURCE, encoding="utf-8")
        (self.fake_bin / "cargo").chmod(0o755)
        self._write_registry()
        self._write_lock()
        self._write_metadata(DARWIN_TARGET, gui=True)
        self._write_metadata(LINUX_TARGET, gui=False)
        self._write_retained_evidence()

    def _write_registry(self):
        source_root = self.cargo_home / "registry" / "src" / REGISTRY_INDEX_DIR
        cache_root = self.cargo_home / "registry" / "cache" / REGISTRY_INDEX_DIR
        cache_root.mkdir(parents=True, exist_ok=True)
        for name, (version, license_id, notice_text) in CRATES.items():
            source_dir = source_root / ("%s-%s" % (name, version))
            (source_dir / "src").mkdir(parents=True, exist_ok=True)
            (source_dir / "Cargo.toml").write_text(CRATE_MANIFEST % (name, version, license_id), encoding="utf-8")
            (source_dir / "src" / "lib.rs").write_text("pub fn stub() {}\n", encoding="utf-8")
            if notice_text is not None:
                (source_dir / "LICENSE").write_text(notice_text, encoding="utf-8")
            if name == "retained-gem":
                # Registry crates carry VCS provenance; the collector uses it for retained reuse.
                (source_dir / ".cargo_vcs_info.json").write_text(
                    json.dumps({"git": {"sha1": RETAINED_REVISION}}), encoding="utf-8")
            archive = crate_archive_bytes(name, version, license_id)
            self.archive_bytes[name] = archive
            self.archive_sha256[name] = sha256_bytes(archive)
            (cache_root / ("%s-%s.crate" % (name, version))).write_bytes(archive)

    def _write_lock(self):
        lines = [
            "# This file is automatically @generated by Cargo.",
            "# It is not intended for manual editing.",
            "version = 3",
            "",
        ]
        for name in sorted(CRATES):
            version = CRATES[name][0]
            lines += [
                "[[package]]",
                'name = "%s"' % name,
                'version = "%s"' % version,
                'source = "%s"' % REGISTRY_SOURCE,
                'checksum = "%s"' % self.archive_sha256[name],
                "",
            ]
        lines += ["[[package]]", 'name = "%s"' % ROOT_NAME, 'version = "%s"' % ROOT_VERSION, ""]
        lock = self.workspace / "Cargo.lock"
        lock.write_text("\n".join(lines), encoding="utf-8")
        self.lock_sha256 = sha256_bytes(lock.read_bytes())

    def _dependency_edges(self, gui):
        edges = list(ROOT_EDGES)
        if not gui:
            edges = [edge for edge in edges if edge[0] != "epsilon-gui"]
        return edges

    def _write_metadata(self, platform, gui):
        edges = self._dependency_edges(gui)
        packages = [self._package_record(ROOT_NAME, edges)]
        for name in sorted(CRATES):
            if name == "epsilon-gui" and not gui:
                continue
            packages.append(self._package_record(name, TRANSITIVE_EDGES.get(name, [])))
        nodes = [{
            "id": self.crate_id(ROOT_NAME),
            "dependencies": [self.crate_id(crate) for crate, _ in edges],
            "deps": [self._resolve_dep(crate, kind) for crate, kind in edges],
            "features": [],
        }]
        for name in sorted(CRATES):
            if name == "epsilon-gui" and not gui:
                continue
            node_edges = TRANSITIVE_EDGES.get(name, [])
            nodes.append({
                "id": self.crate_id(name),
                "dependencies": [self.crate_id(crate) for crate, _ in node_edges],
                "deps": [self._resolve_dep(crate, kind) for crate, kind in node_edges],
                "features": [],
            })
        document = {
            "packages": packages,
            "workspace_members": [self.crate_id(ROOT_NAME)],
            "resolve": {"nodes": nodes, "root": self.crate_id(ROOT_NAME)},
            "target_directory": str(self.workspace / "target"),
            "version": 1,
            "workspace_root": str(self.workspace),
        }
        path = self.workspace / "tools" / ("platform-%s.json" % platform)
        path.write_text(json.dumps(document, indent=1), encoding="utf-8")

    def _resolve_dep(self, crate, kind):
        return {"name": crate, "pkg": self.crate_id(crate), "dep_kinds": [{"kind": kind, "target": None}]}

    def _package_record(self, name, edges):
        if name == ROOT_NAME:
            manifest = self.workspace / "Cargo.toml"
            license_id = "Apache-2.0"
            source = None
            repository = None
        else:
            version, license_id, _ = CRATES[name]
            manifest = self.cargo_home / "registry" / "src" / REGISTRY_INDEX_DIR / ("%s-%s" % (name, version)) / "Cargo.toml"
            source = REGISTRY_SOURCE
            repository = "https://github.com/fixture-labs/retained-gem" if name == "retained-gem" else None
        record = {
            "name": name,
            "version": ROOT_VERSION if name == ROOT_NAME else CRATES[name][0],
            "id": self.crate_id(name),
            "license": license_id,
            "license_file": None,
            "description": "synthetic fixture package",
            "source": source,
            "repository": repository,
            "dependencies": [
                {"name": crate, "source": REGISTRY_SOURCE, "req": "*", "kind": kind, "rename": None,
                 "optional": False, "uses_default_features": True, "features": [],
                 "target": None, "registry": None}
                for crate, kind in edges
            ],
            "targets": [{"kind": ["lib"], "name": name, "src_path": str(manifest.parent / "src/lib.rs")}],
            "features": {},
            "manifest_path": str(manifest),
        }
        return record

    def _write_retained_evidence(self):
        notices = self.workspace / "vendor" / "retrieval-kernel" / "notices"
        entry_dir = notices / "retained-gem-0.9.0"
        entry_dir.mkdir(parents=True, exist_ok=True)
        license_file = entry_dir / "LICENSE"
        license_file.write_text(LICENSE_RETAINED, encoding="utf-8")
        self.retained_notice_sha256 = sha256_bytes(LICENSE_RETAINED.encode("utf-8"))
        upstream = [{
            "crate": "retained-gem",
            "version": "0.9.0",
            "file": "retained-gem-0.9.0/LICENSE",
            # The URL is arbitrary: identity matching keys on crate/version/VCS revision and hash.
            "url": "https://git.example.org/arbitrary/attachment.bin",
            "revision": RETAINED_REVISION,
            "sha256": self.retained_notice_sha256,
        }]
        (notices / "UPSTREAM.json").write_text(json.dumps(upstream, indent=1), encoding="utf-8")

    # -- operation ---------------------------------------------------------

    def run_collector(self, subject, output, record_name, extra_args=()):
        """Run the exported subject file as the real collector CLI with the synthetic Cargo effect."""
        record = self.root / record_name
        environment = {
            "PATH": str(self.fake_bin) + os.pathsep + os.environ.get("PATH", ""),
            "HOME": str(self.root / "home"),
            "CARGO_HOME": str(self.cargo_home),
            "FAKE_CARGO_RECORD": str(record),
            "FAKE_CARGO_METADATA": str(self.workspace / "tools"),
        }
        process = subprocess.run(
            [sys.executable, "-B", str(subject.path), *extra_args, str(output)],
            env=environment, cwd=str(self.root),
            capture_output=True, text=True, timeout=COLLECT_RUN_TIMEOUT_SECONDS,
        )
        return process, output, record

    def corrupt_lock_checksum(self, crate, replacement):
        lock = self.workspace / "Cargo.lock"
        text = lock.read_text(encoding="utf-8")
        pattern = re.compile(
            r'(\[\[package\]\]\nname = "%s"\nversion = "[^"]+"\nsource = "[^"]+"\nchecksum = ")([0-9a-f]{64})(")' % crate
        )
        corrupted, count = pattern.subn(lambda match: match.group(1) + replacement + match.group(3), text)
        if count != 1:
            raise AssertionError("fixture lock corruption did not find exactly one checksum for %s" % crate)
        lock.write_text(corrupted, encoding="utf-8")

    def redeclare_retained_hash(self, digest):
        notices = self.workspace / "vendor" / "retrieval-kernel" / "notices"
        upstream = notices / "UPSTREAM.json"
        entries = json.loads(upstream.read_text(encoding="utf-8"))
        entries[0]["sha256"] = digest
        upstream.write_text(json.dumps(entries, indent=1), encoding="utf-8")


class NoticeCollectorCharacterization(unittest.TestCase):
    """One primary representative case per owner-contract responsibility."""

    maxDiff = None

    def fresh_fixture(self):
        return Fixture(Path(tempfile.mkdtemp(prefix=self._testMethodName + "-", dir=ADMISSION_ROOT)))

    def package_map(self, inventory):
        return {package["name"]: package for package in inventory["packages"]}

    def selection(self, inventory):
        return {(package["name"], package["version"]) for package in inventory["packages"]}

    def read_inventory(self, output):
        return json.loads((Path(output) / "inventory.json").read_text(encoding="utf-8"))

    def expected_selection(self, names):
        return {(name, CRATES[name][0]) for name in names}

    def assert_nonempty_warning_classification(self, warnings):
        """Assert the inventory keeps explicit nonempty coverage warnings without binding their English wording."""
        self.assertTrue(warnings, "coverage gaps must stay explicitly flagged in the inventory")
        for warning in warnings:
            self.assertIsInstance(warning, str)
            self.assertTrue(warning.strip(), "each coverage warning must carry nonempty text")

    def document_section(self, document, crate, version):
        """Return the notice-document section between the crate header and the next section header."""
        lines = document.splitlines()
        start = lines.index("=== %s %s ===" % (crate, version))
        stop = next((index for index in range(start + 1, len(lines)) if lines[index].startswith("=== ")), len(lines))
        return "\n".join(lines[start:stop])

    # -- explicit Linux/headless binding and selection ----------------------

    def test_linux_headless_binds_inventory_and_selects_normal_build(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        process, output, record = fixture.run_collector(
            subject, fixture.root / "out", "calls-linux.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(calls, [[
            "metadata", "--offline", "--locked", "--filter-platform", LINUX_TARGET,
            "--format-version", "1", "--no-default-features",
        ]])
        inventory = self.read_inventory(output)
        self.assertEqual(inventory["target"], LINUX_TARGET)
        self.assertEqual(inventory["features"], [])
        self.assertEqual(inventory["root_package"], {"name": ROOT_NAME, "version": ROOT_VERSION})
        self.assertEqual(inventory["lockfile_sha256"], fixture.lock_sha256)
        self.assertEqual(inventory["package_count"], 5)
        self.assertEqual(inventory["declared_license_counts"], {"Apache-2.0": 2, "MIT": 2, "MPL-2.0": 1})
        self.assertEqual(
            self.selection(inventory),
            self.expected_selection(["alpha-core", "beta-build", "gamma-transitive", "mpl-lib", "retained-gem"]),
        )
        self.assertNotIn("delta-dev", self.package_map(inventory), "dev-only edges must stay excluded")

    def test_retained_notice_reused_with_matching_identity_and_hash(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        process, output, _ = fixture.run_collector(
            subject, fixture.root / "out", "calls-retained.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        package = self.package_map(self.read_inventory(output))["retained-gem"]
        self.assertEqual(package["declared_license"], "Apache-2.0")
        self.assertEqual(package["notices"], [{
            "path": "retained-upstream/retained-gem-0.9.0/LICENSE",
            "text_sha256": fixture.retained_notice_sha256,
        }])
        self.assert_nonempty_warning_classification(package["warnings"])
        self.assertIsNone(package.get("source_archive"))
        artifact_archives = {path.name for path in (Path(output) / "Sources").iterdir()}
        self.assertNotIn("retained-gem-0.9.0.crate", artifact_archives)
        document = (Path(output) / "THIRD_PARTY_NOTICES.txt").read_text(encoding="utf-8")
        self.assertIn("notices: retained-upstream/retained-gem-0.9.0/LICENSE -> full text SHA-256 %s"
                      % fixture.retained_notice_sha256, document)
        self.assertIn("=== Full upstream text SHA-256 %s ===" % fixture.retained_notice_sha256, document)
        self.assertIn(LICENSE_RETAINED.strip("\n"), document)

    def test_notice_gap_keeps_exact_source_archive_and_reports_honestly(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        process, output, _ = fixture.run_collector(
            subject, fixture.root / "out", "calls-gap.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        package = self.package_map(self.read_inventory(output))["gamma-transitive"]
        expected_archive = fixture.archive_bytes["gamma-transitive"]
        expected_digest = fixture.archive_sha256["gamma-transitive"]
        self.assertEqual(package["declared_license"], "MIT")
        self.assertEqual(package["notices"], [])
        self.assert_nonempty_warning_classification(package["warnings"])
        self.assertEqual(package["source_archive"], {
            "path": "Sources/gamma-transitive-0.5.0.crate",
            "sha256": expected_digest,
            "bytes": len(expected_archive),
        })
        self.assertEqual(package["source_archive_reason"], "upstream_notice_gap_evidence")
        self.assertEqual(package["versioned_source_url"],
                         "https://crates.io/api/v1/crates/gamma-transitive/0.5.0/download")
        archived = (Path(output) / "Sources" / "gamma-transitive-0.5.0.crate").read_bytes()
        self.assertEqual(archived, expected_archive, "archive bytes must equal the lock-checksummed original")
        document = (Path(output) / "THIRD_PARTY_NOTICES.txt").read_text(encoding="utf-8")
        self.assertIn("Included unmodified source (upstream_notice_gap_evidence): "
                      "Sources/gamma-transitive-0.5.0.crate (SHA-256 %s)" % expected_digest, document)
        section = self.document_section(document, "gamma-transitive", "0.5.0")
        self.assertTrue(
            any(line[len("Coverage note:"):].strip() for line in section.splitlines() if line.startswith("Coverage note:")),
            "the notice document must keep an explicit nonempty coverage note in the affected crate section",
        )
        self.assertNotIn("=== Full upstream text SHA-256 %s ===" % expected_digest, document,
                         "a notice gap must not gain invented upstream text")

    def test_mpl_source_obligation_keeps_original_archive(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        process, output, _ = fixture.run_collector(
            subject, fixture.root / "out", "calls-mpl.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        package = self.package_map(self.read_inventory(output))["mpl-lib"]
        expected_archive = fixture.archive_bytes["mpl-lib"]
        expected_digest = fixture.archive_sha256["mpl-lib"]
        self.assertEqual(package["declared_license"], "MPL-2.0")
        self.assertEqual(package["notices"], [])
        self.assertEqual(package["source_archive"], {
            "path": "Sources/mpl-lib-1.2.3.crate",
            "sha256": expected_digest,
            "bytes": len(expected_archive),
        })
        self.assertEqual(package["source_archive_reason"], "mpl_source_obligation")
        archived = (Path(output) / "Sources" / "mpl-lib-1.2.3.crate").read_bytes()
        self.assertEqual(archived, expected_archive)
        document = (Path(output) / "THIRD_PARTY_NOTICES.txt").read_text(encoding="utf-8")
        self.assertIn("Included unmodified source (mpl_source_obligation): "
                      "Sources/mpl-lib-1.2.3.crate (SHA-256 %s)" % expected_digest, document)

    # -- preserved default Darwin GUI selection ------------------------------

    def test_default_darwin_gui_selection_preserved(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        process, output, record = fixture.run_collector(subject, fixture.root / "out", "calls-default.jsonl")
        self.assertEqual(process.returncode, 0, process.stderr)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(calls, [[
            "metadata", "--offline", "--locked", "--filter-platform", DARWIN_TARGET,
            "--format-version", "1", "--features", "gui",
        ]])
        inventory = self.read_inventory(output)
        self.assertEqual(inventory["target"], DARWIN_TARGET)
        self.assertEqual(inventory["features"], ["gui"])
        self.assertEqual(inventory["root_package"], {"name": ROOT_NAME, "version": ROOT_VERSION})
        self.assertEqual(inventory["lockfile_sha256"], fixture.lock_sha256)
        self.assertEqual(
            self.selection(inventory),
            self.expected_selection([
                "alpha-core", "beta-build", "epsilon-gui", "gamma-transitive", "mpl-lib", "retained-gem",
            ]),
        )
        self.assertNotIn("delta-dev", self.package_map(inventory), "dev-only edges must stay excluded")

    def test_default_selection_matches_older_committed_source(self):
        fixture = self.fresh_fixture()
        current, legacy = legacy_subjects()
        new_process, new_output, _ = fixture.run_collector(current, fixture.root / "out-new", "calls-new.jsonl")
        old_process, old_output, old_record = fixture.run_collector(legacy, fixture.root / "out-old", "calls-old.jsonl")
        self.assertEqual(new_process.returncode, 0, new_process.stderr)
        self.assertEqual(old_process.returncode, 0, old_process.stderr)
        old_calls = [json.loads(line) for line in old_record.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(len(old_calls), 1)
        self.assertIn("--filter-platform", old_calls[0])
        self.assertIn(DARWIN_TARGET, old_calls[0])
        self.assertIn("--features", old_calls[0])
        self.assertIn("gui", old_calls[0])
        new_inventory = self.read_inventory(new_output)
        old_inventory = self.read_inventory(old_output)
        self.assertEqual(self.selection(old_inventory), self.selection(new_inventory))
        self.assertEqual(old_inventory["target"], new_inventory["target"])
        self.assertEqual(old_inventory["features"], new_inventory["features"])
        # The older source predates inventory binding; this records the difference instead of claiming an old pass for the newer Linux option.
        self.assertNotIn("lockfile_sha256", old_inventory)
        self.assertNotIn("root_package", old_inventory)

    # -- material negative controls ------------------------------------------

    def test_wrong_retained_hash_is_refused(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        wrong_digest = sha256_bytes(TAMPERED_RETAINED_BYTES)
        fixture.redeclare_retained_hash(wrong_digest)
        output = fixture.root / "out"
        process, output, _ = fixture.run_collector(
            subject, output, "calls-retained-wrong.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("retained notice checksum mismatch: retained-gem-0.9.0/LICENSE", process.stderr)
        self.assertEqual(sorted(path.name for path in Path(output).iterdir()), [],
                         "a refused run must write no artifact into the destination")

    def test_lock_checksum_mismatch_is_refused(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        fixture.corrupt_lock_checksum("gamma-transitive", "ab" * 32)
        output = fixture.root / "out"
        process, output, _ = fixture.run_collector(
            subject, output, "calls-lock-wrong.jsonl",
            extra_args=["--target", LINUX_TARGET, "--headless"],
        )
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("crate source archive checksum mismatch: gamma-transitive-0.5.0.crate", process.stderr)
        self.assertEqual(sorted(path.name for path in Path(output).iterdir()), [],
                         "a refused run must write no artifact into the destination")

    def test_existing_destination_refused_without_overwrite(self):
        fixture = self.fresh_fixture()
        subject = current_subject()
        output = fixture.root / "out"
        output.mkdir()
        sentinel_inventory = b"sentinel inventory bytes\n"
        sentinel_keep = b"keep me\n"
        (output / "inventory.json").write_bytes(sentinel_inventory)
        (output / "KEEP.txt").write_bytes(sentinel_keep)
        process, _, _ = fixture.run_collector(subject, output, "calls-destination.jsonl")
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("File exists", process.stderr)
        self.assertEqual((output / "inventory.json").read_bytes(), sentinel_inventory)
        self.assertEqual((output / "KEEP.txt").read_bytes(), sentinel_keep)
        self.assertEqual(sorted(path.name for path in output.iterdir()), ["KEEP.txt", "inventory.json"])


def parse_arguments():
    """Define the subject-selection command line for the characterization run."""
    parser = argparse.ArgumentParser(description="Characterization tests for the license-notice collector subject.")
    parser.add_argument(
        "--legacy", action="store_true",
        help="run the pinned f80813f/a4d77f0 historical characterization including the OLD/NEW parity test",
    )
    parser.add_argument(
        "--subject-repo", default=os.environ.get(SUBJECT_REPO_ENV),
        help="checkout holding the subject source; default NOTICE_COLLECTION_SUBJECT_REPO, else this checkout "
             "(legacy search also tries the sibling lead checkout)",
    )
    parser.add_argument(
        "--current-ref", default=None,
        help="ref resolved to the current subject commit; default HEAD; not valid with --legacy",
    )
    return parser


def main():
    parser = parse_arguments()
    arguments = parser.parse_args()
    if arguments.legacy:
        if arguments.current_ref is not None:
            parser.error("--current-ref does not apply to legacy mode; legacy pins its declared revisions")
        SELECTED["legacy_repo"] = arguments.subject_repo
        subject = export_pinned_subject(LEGACY_NEW_COMMIT, LEGACY_NEW_SHA256, arguments.subject_repo)
        select_current_subject(subject)
        print("legacy mode: pinned subject %s sha256 %s from %s" % (subject.commit, subject.sha256, subject.repository))
    else:
        repository = Path(arguments.subject_repo) if arguments.subject_repo else REPO_ROOT
        ref = arguments.current_ref if arguments.current_ref is not None else "HEAD"
        subject = resolve_current_subject(repository, ref)
        select_current_subject(subject)
        print("current mode: repo %s ref %s -> commit %s sha256 %s"
              % (subject.repository, subject.ref, subject.commit, subject.sha256))
    ADMISSION_ROOT.mkdir(parents=True, exist_ok=True)
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(NoticeCollectorCharacterization)
    if not arguments.legacy:
        cases = list(suite)
        kept = [case for case in cases if case._testMethodName not in LEGACY_ONLY_TESTS]
        dropped = sorted(case._testMethodName for case in cases if case._testMethodName in LEGACY_ONLY_TESTS)
        suite = unittest.TestSuite(kept)
        print("legacy-only tests not selected in current mode: %s" % (", ".join(dropped) if dropped else "(none)"))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if result.wasSuccessful():
        # Disposable synthetic fixtures; failing runs stay on disk for inspection.
        shutil.rmtree(ADMISSION_ROOT, ignore_errors=True)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
