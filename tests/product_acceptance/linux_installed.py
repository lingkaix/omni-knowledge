#!/usr/bin/env python3
"""Accept an actual Linux headless archive from an isolated, owned install prefix.

No Cargo, source-tree harness, provider, or source executable is used. Receipts qualify
only the installed init, native Query/Refresh, MCP, lexical runtime and parser sibling
paths exercised here. --self-test admits the process observer, not the product.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile


class Failure(Exception):
    """A real prerequisite, observation or product assertion failed."""


def require(condition, message):
    if not condition:
        raise Failure(message)


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def identity(pid):
    """Read only one exact owned process identity; birth prevents PID-reuse confusion."""
    try:
        fields = (Path("/proc") / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
        return fields[19]
    except (FileNotFoundError, ProcessLookupError):
        return None


def descendants(pid):
    pending, found = [pid], {}
    while pending:
        current = pending.pop()
        birth = identity(current)
        if birth is None or current in found:
            continue
        found[current] = birth
        try:
            for task in (Path("/proc") / str(current) / "task").iterdir():
                pending.extend(map(int, (task / "children").read_text().split()))
        except (FileNotFoundError, ProcessLookupError):
            pass
    return found


def kill_owned(rows):
    """Signal only exact fixture-owned identities with pidfds, never a name/group search."""
    for pid, birth in reversed(list(rows.items())):
        try:
            descriptor = os.pidfd_open(pid)
            try:
                if identity(pid) == birth:
                    signal.pidfd_send_signal(descriptor, signal.SIGKILL)
            finally:
                os.close(descriptor)
        except ProcessLookupError:
            pass


class Runner:
    """Finite foreground command observation with immutable per-command raw logs."""
    def __init__(self, evidence, cwd, env, allowed_executables=None):
        self.evidence, self.cwd, self.env = evidence, cwd, env
        self.allowed_executables = allowed_executables
        self.receipts = []

    def run(self, label, command, data=None, expected=0, timeout=180):
        stdout = self.evidence / (label + ".stdout")
        stderr = self.evidence / (label + ".stderr")
        stdin = self.evidence / (label + ".stdin")
        stdin.write_bytes(b"" if data is None else data)
        observed, executables = {}, {}
        started = time.monotonic()
        status = "PASS"
        with stdin.open("rb") as source, stdout.open("xb") as out, stderr.open("xb") as err:
            child = subprocess.Popen(list(map(str, command)), cwd=self.cwd, env=self.env,
                                     stdin=source, stdout=out, stderr=err)
            root_birth = identity(child.pid)
            try:
                while True:
                    rows = descendants(child.pid)
                    for pid, birth in rows.items():
                        observed[pid] = birth
                        try:
                            executable = os.readlink(f"/proc/{pid}/exe")
                            executables[(pid, birth)] = executable
                        except (FileNotFoundError, ProcessLookupError):
                            pass
                    code = child.poll()
                    if code is not None:
                        break
                    if time.monotonic() - started >= timeout:
                        status = "TIMEOUT"
                        kill_owned(observed)
                        child.wait()
                        code = child.returncode
                        break
                    time.sleep(0.003)
            finally:
                if child.poll() is None:
                    kill_owned(observed | {child.pid: root_birth})
                    child.wait()
            survivors = {pid: birth for pid, birth in observed.items()
                         if pid != child.pid and identity(pid) == birth}
            if survivors:
                kill_owned(survivors)
                status = "FAIL"
            allowed = expected if isinstance(expected, tuple) else (expected,)
            if code not in allowed and status == "PASS":
                status = "FAIL"
            unexpected = sorted({exe for exe in executables.values()
                                 if self.allowed_executables is not None and exe not in self.allowed_executables})
            if unexpected:
                status = "FAIL"
        receipt = {"label": label, "command": list(map(str, command)), "cwd": str(self.cwd),
                   "status": status, "exit": code, "expected_exit": expected,
                   "elapsed_seconds": time.monotonic() - started,
                   "root_pid": child.pid, "root_birth": root_birth, "root_collected": True,
                   "observed_children_collected": not survivors,
                   "unexpected_observed_executables": unexpected,
                   "observed_executables": [{"pid": pid, "birth": birth, "exe": exe}
                                            for (pid, birth), exe in executables.items()]}
        save(self.evidence / (label + ".json"), receipt)
        self.receipts.append(receipt)
        require(status == "PASS", f"{label}: {status}; inspect retained raw logs")
        return stdout.read_bytes()


def unpack(artifact, destination):
    """Extract regular files/directories into a newly owned prefix, without following links."""
    seen = set()
    with tarfile.open(artifact, "r:gz") as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            require(not name.is_absolute() and ".." not in name.parts, "archive path escapes prefix")
            require(member.isdir() or member.isfile(), "archive contains a link or special file")
            require(name not in seen, "archive contains a duplicate path")
            seen.add(name)
            path = destination.joinpath(*name.parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source, path.open("xb") as output:
                    shutil.copyfileobj(source, output)
                path.chmod(0o644 | (member.mode & 0o111))
    manifests = list(destination.glob("*/manifest.json"))
    require(len(manifests) == 1, "archive must contain one headless product root")
    return manifests[0].parent


def verify(stage):
    manifest = json.loads((stage / "manifest.json").read_text())
    require(manifest["kind"] == "headless-product-archive", "wrong artifact kind")
    require(manifest["target"] == "x86_64-unknown-linux-gnu" and manifest["features"] == [],
            "artifact does not match the accepted Linux headless host")
    binaries = {}
    for item in manifest["bins"]:
        name = item["installed_name"]
        require(name in {"omni", "parser-worker"} and name == item["name"], "unexpected installed bin")
        path = stage / name
        require(path.is_file() and not path.is_symlink() and os.access(path, os.X_OK), "missing executable")
        require(sha(path) == item["sha256"] and path.stat().st_size == item["bytes"], "binary hash/size mismatch")
        require(name not in binaries, "duplicate binary manifest entry")
        binaries[name] = item["sha256"]
    require(set(binaries) == {"omni", "parser-worker"}, "actual product/parser sibling missing")
    require(len(set(binaries.values())) == 2, "parser is a cloned product binary")
    require(manifest["parser_helper"]["status"] == "installed", "parser lookup not declared")
    require(not (stage / "omni-fsevents").exists(), "Linux artifact cannot qualify FSEvents")
    require(manifest["license"]["path"] == "LICENSE", "license path is not the declared product license")
    require(sha(stage / manifest["license"]["path"]) == manifest["license"]["sha256"], "license hash mismatch")
    inventory = json.loads((stage / "ThirdParty/inventory.json").read_text())
    require(inventory["lockfile_sha256"] == manifest["lockfile_sha256"], "notice lock binding differs")
    require(inventory["target"] == manifest["target"] and inventory["features"] == [], "notice target differs")
    notices = (stage / "ThirdParty/THIRD_PARTY_NOTICES.txt").read_bytes().decode("utf-8")
    texts = {digest: text for digest, text in re.findall(
        r"=== Full upstream text SHA-256 ([0-9a-f]{64}) ===\n(.*?)\n=== End upstream text ===", notices, re.S)}
    for digest, text in texts.items():
        require(hashlib.sha256(text.encode()).hexdigest() == digest, "notice text hash mismatch")
    for package in inventory["packages"]:
        for notice in package["notices"] + package["fallback_evidence"]:
            require(notice["text_sha256"] in texts, "inventoried notice text missing")
        if package.get("source_archive"):
            entry = package["source_archive"]
            relative = PurePosixPath(entry["path"])
            require(not relative.is_absolute() and ".." not in relative.parts, "source notice path escapes")
            require(sha(stage / "ThirdParty" / entry["path"]) == entry["sha256"], "source notice archive hash differs")
    return {"manifest_sha256": sha(stage / "manifest.json"), "bins": binaries,
            "lockfile_sha256": manifest["lockfile_sha256"], "notice_packages": len(inventory["packages"]),
            "notice_texts_verified": len(texts), "notice_coverage_limit": inventory["coverage_limit"],
            "dependency_versions": {p["name"]: p["version"] for p in inventory["packages"]}}


def paths(search, library_id):
    require(len(search["groups"]) == 1, "expected one selected installed library")
    group = search["groups"][0]
    require(group["library_id"] == library_id and group["generation_id"], "actual library/generation label missing")
    require(group["coverage"]["enumeration"] == "complete", "installed enumeration is incomplete")
    require(group["returned_files"] == len(group["files"]), "returned file accounting differs")
    return {hit["source"]["path"] for hit in group["files"]}


def docx(path, word):
    with zipfile.ZipFile(path, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("word/document.xml", '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>' + word + ' ' + 'synthetic parser filler ' * 2000 + '</w:t></w:r></w:p></w:body></w:document>')


def actual(artifact, evidence):
    require(platform.system() == "Linux" and platform.machine() == "x86_64", "BLOCKED: requires dev Linux x86_64")
    work = Path(tempfile.mkdtemp(prefix="installed-", dir=evidence))
    prefix = work / "prefix"
    prefix.mkdir()
    stage = unpack(artifact, prefix)
    archive_proof = verify(stage)
    for name in ["home", "os-cache", "temporary", "execution", "tool-path"]:
        (work / name).mkdir()
    for name in ["ffmpeg", "ffprobe"]:
        executable = shutil.which(name, path="/usr/bin:/bin")
        require(executable, "BLOCKED: actual system media executables missing")
        (work / "tool-path" / name).symlink_to(Path(executable).resolve())
    env = {"HOME": str(work / "home"), "XDG_CACHE_HOME": str(work / "os-cache"),
           "TMPDIR": str(work / "temporary"), "OMNI_CACHE_DIR": str(work / "native-cache"),
           "PATH": str(work / "tool-path"), "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8"}
    binary = stage / "omni"
    allowed_executables = {str(binary), str(stage / "parser-worker")}
    allowed_executables.update(str((work / "tool-path" / name).resolve()) for name in ["ffmpeg", "ffprobe"])
    runner = Runner(evidence, work / "execution", env, allowed_executables)
    version = runner.run("version", [binary, "--version"]).decode().strip()
    runner.run("help", [binary, "--help"])
    doctor = json.loads(runner.run("doctor", [binary, "doctor", "--json"]))
    require(doctor["ffmpeg"] and doctor["ffprobe"], "installed doctor cannot find required tools")
    library = work / "library"
    runner.run("init", [binary, "init", library, "--name", "Installed synthetic acceptance", "--json"])
    library_id = json.loads((library / "omni-knowledge.json").read_text())["library_id"]
    markdown = library / "pages/installed.md"
    markdown.write_text("---\ntags: [InstallTag, Both]\n---\n# Installed synthetic\ninstallalpha 日本語\n")
    office = library / "raw/installed.docx"
    docx(office, "installeddocxword")

    def search(label, keyword, mode="bm25", tags=()):
        args = [binary, "--root", library, "search", keyword, "--mode", mode, "--canonical", "--json", "--limit", "5"]
        for tag in tags:
            args.extend(["--tag", tag])
        return json.loads(runner.run(label, args))

    lexical = search("cold-lexical", "installalpha", tags=["installtag", "Both"])
    require(paths(lexical, library_id) == {"pages/installed.md"}, "cold native lexical query lost source/tags")
    source = lexical["groups"][0]["files"][0]["source"]
    require(source["digest"] == sha(markdown), "installed query does not bind actual source bytes")
    japanese = search("lexical-dictionary", "日本語", tags=["InstallTag", "Both"])
    require(paths(japanese, library_id) == {"pages/installed.md"}, "installed multilingual lexical runtime failed")
    parsed = search("parser-positive", "installeddocxword", mode="literal")
    parser_ok = paths(parsed, library_id) == {"raw/installed.docx"}
    failures = []
    blocked_checks = []
    if parser_ok:
        require(parsed["groups"][0]["files"][0]["source"]["digest"] == sha(office), "parser result lost original digest")
        # The new source has NEVER had a prior representation: cached old text cannot satisfy this control.
        pending = library / "raw/required-sibling.docx"
        docx(pending, "missingsiblingword")
        helper = stage / "parser-worker"
        hidden = stage / "parser-worker.disabled"
        helper.rename(hidden)
        try:
            args = [binary, "--root", library, "search", "missingsiblingword", "--mode", "literal", "--canonical", "--json"]
            output = runner.run("parser-absent", args, expected=(0, 1))
            runner.receipts[-1]["role"] = "declared missing-sibling negative control"
            if runner.receipts[-1]["exit"] == 0:
                missing = json.loads(output)
                require("raw/required-sibling.docx" not in paths(missing, library_id), "absent installed helper used a fallback")
                require(int(missing["groups"][0]["coverage"]["failed"]) > 0, "missing helper was hidden as successful empty coverage")
            else:
                require(not output, "missing sibling refusal emitted partial success")
                failure = json.loads((evidence / "parser-absent.stderr").read_bytes())
                require(isinstance(failure.get("error"), str), "missing sibling did not report a normal CLI error")

        finally:
            hidden.rename(helper)
        restored = search("parser-restored", "missingsiblingword", mode="literal")
        require(paths(restored, library_id) == {"raw/required-sibling.docx"}, "restoring installed sibling did not restore extraction")
    else:
        failures.append("installed native producer did not extract actual DEFLATE DOCX; parser-positive raw result retained")
        blocked_checks.extend(["parser absent/restored control requires the unchanged positive DOCX oracle"])

    def query(methods, keywords):
        return {"query": {"library_ids": [library_id], "keywords": keywords, "methods": methods,
                          "required_tags": ["InstallTag", "Both"], "case_sensitive": False, "limit": 5,
                          "image_query": None, "budget": {"max_files": 1000, "max_bytes": "67108864",
                          "max_millis": 60000, "max_memory_bytes": "536870912"}}}

    requests = [
        ("initialize", {"protocolVersion": "2025-11-25"}),
        ("tools/list", {}), ("tools/call", {"name": "library_list", "arguments": {}}),
    ]
    for methods, keywords in [(["bm25", "literal"], ["installalpha"]), (["regex"], ["["]),
                              (["semantic"], ["installalpha"]), (["bm25", "literal"], ["installalpha"])]:
        requests.append(("tools/call", {"name": "query", "arguments": query(methods, keywords)}))
    data = b"".join((json.dumps({"jsonrpc": "2.0", "id": index + 1, "method": method, "params": params}, ensure_ascii=False) + "\n").encode()
                    for index, (method, params) in enumerate(requests))
    replies = [json.loads(line) for line in runner.run("canonical-mcp", [binary, "--root", library, "serve", "--stdio"], data=data).splitlines()]
    require(len(replies) == len(requests) and [r["id"] for r in replies] == list(range(1, len(requests) + 1)), "installed MCP lost/reordered replies")
    require(replies[0]["result"]["protocolVersion"] == "2025-11-25", "wrong canonical protocol")
    catalog = {tool["name"] for tool in replies[1]["result"]["tools"]}
    require("query" in catalog and "exposure_set" not in catalog and "library_open" not in catalog, "MCP owner-only catalog widened")
    native = []
    for reply in replies[2:]:
        result = reply["result"]
        decoded = json.loads(result["content"][0]["text"])
        require(decoded == result["structuredContent"], "MCP text and structured result differ")
        require(result["isError"] == (decoded["result"]["type"] == "error"), "MCP error flag differs from typed result")
        native.append(decoded["result"])
    require(native[0]["libraries"][0]["library_id"] == library_id and native[0]["libraries"][0]["exposure"]["mode"] == "read_only", "MCP did not use actual selected read-only library")
    require(paths(native[1]["search"], library_id) == {"pages/installed.md"}, "installed MCP query failed")
    for result, expected_code, method in [(native[2], "invalid_request", "regex"), (native[3], "provider_unavailable", "semantic")]:
        require(result["type"] == "error" and result["error"]["code"] == expected_code and result["error"]["method"] == method, "installed typed method failure lost")
    require(native[1] == native[4], "failed/repeated installed queries changed prior result or generation")
    legacy_requests = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "search", "arguments": {"query": "installeddocxword", "mode": "grep"}}},
        {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "search", "arguments": {"query": "installalpha", "mode": "bm25"}}},
    ]
    data = ("\n".join(map(json.dumps, legacy_requests)) + "\n").encode()
    legacy = [json.loads(line) for line in runner.run("legacy-mcp", [binary, "--root", library, "serve-mcp"], data=data).splitlines()]
    require(legacy[0]["result"]["protocolVersion"] == "2024-11-05" and len(legacy[1]["result"]["tools"]) == 7, "legacy MCP contract changed")
    legacy_docx = json.loads(legacy[2]["result"]["content"][0]["text"])
    if not legacy_docx or legacy_docx[0]["path"] != "raw/installed.docx":
        failures.append("legacy installed DOCX Query has the same failed native representation")
    require(legacy[3]["result"]["isError"] is False and json.loads(legacy[3]["result"]["content"][0]["text"])[0]["path"] == "pages/installed.md", "independent legacy MCP lexical Query failed")
    require(verify(stage) == archive_proof, "installed product changed during acceptance")
    return {"status": "FAIL" if failures else "PASS", "failures": failures, "blocked_checks": blocked_checks,
            "artifact": str(artifact), "artifact_sha256": sha(artifact), "product_version": version,
            "install_prefix": str(prefix), "cwd": str(runner.cwd), "path": env["PATH"], "archive": archive_proof,
            "checks": {"archive_hashes_notices": "PASS", "init": "PASS", "cold_native_lexical": "PASS",
                       "japanese_bm25": "PASS", "canonical_mcp": "PASS", "legacy_lexical_mcp": "PASS",
                       "native_docx": "PASS" if parser_ok else "FAIL",
                       "parser_absent_restored": "PASS" if parser_ok else "BLOCKED"},
            "receipts": runner.receipts, "parser_lookup": "new-source absent/refused then same-source restored/success" if parser_ok else "BLOCKED by failed unchanged positive DOCX oracle",
            "observed_parser_pids": sorted({row["pid"] for receipt in runner.receipts for row in receipt["observed_executables"] if row["exe"] == str(stage / "parser-worker")}),
            "dictionary": "actual Japanese BM25 in fresh HOME/cache without build dictionary environment",
            "qualification": "installed Linux init/native Refresh/Query/CLI/MCP/parser; model weights, macOS and physical-resource qualification remain separate"}


def self_test(evidence):
    work = Path(tempfile.mkdtemp(prefix="instrument-", dir=evidence))
    runner = Runner(evidence, work, {"PATH": "", "LANG": "C.UTF-8"})
    require(runner.run("admit-pass", [sys.executable, "-c", "print('entered')"], timeout=5) == b"entered\n", "success observer lost output")
    for label, program, timeout, expected in [
        ("admit-fail", "print('entered',flush=True);raise SystemExit(7)", 5, "FAIL"),
        ("admit-timeout", "import time;print('entered',flush=True);time.sleep(30)", 1, "TIMEOUT"),
    ]:
        try:
            runner.run(label, [sys.executable, "-c", program], timeout=timeout)
        except Failure:
            require(runner.receipts[-1]["status"] == expected, "observer misclassified deliberate failure")
            require((evidence / (label + ".stdout")).read_text() == "entered\n", "timeout/failure did not reach deciding path")
        else:
            raise Failure("observer accepted a deliberate failure/timeout")
    require(all(r["root_collected"] and r["observed_children_collected"] for r in runner.receipts), "instrument retained children")
    return {"status": "PASS", "qualification": "instrument admission only", "receipts": runner.receipts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--evidence-dir", required=True, type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.evidence_dir.is_absolute() or (not args.self_test and (args.artifact is None or not args.artifact.is_absolute())):
        parser.error("artifact and evidence-dir must be absolute paths")
    args.evidence_dir.mkdir(parents=True, mode=0o700, exist_ok=False)
    try:
        require(platform.system() == "Linux" and hasattr(os, "pidfd_open"), "BLOCKED: native Linux process observation required")
        if args.self_test:
            result = self_test(args.evidence_dir)
        else:
            require(args.artifact.is_file() and not args.artifact.is_symlink(), "BLOCKED: actual archive is missing")
            result = actual(args.artifact, args.evidence_dir)
    except (Failure, OSError, ValueError, KeyError, tarfile.TarError) as error:
        result = {"status": "BLOCKED" if str(error).startswith("BLOCKED:") else "FAIL", "error": str(error)}
    save(args.evidence_dir / "result.json", result)
    print(json.dumps({"status": result["status"], "evidence": str(args.evidence_dir)}))
    return 0 if result["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
