//! Native carrier and projection controls through public APIs and an independent POSIX TAR library.
use super::super::*;
use knowledge_package::{
    V1Carrier, V1ExportOutcome, V1File, V1Package, V1PathMap, V1Projection, V1ReferenceMap,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const OLD: &str = "ks_019aa000-0000-7000-8000-000000000001";
const NEW: &str = "ks_019aa000-0000-7000-8000-000000000002";

/// Compute independently expected file identities from actual normal-data fixture bytes.
fn selected(path: &str, bytes: &[u8]) -> V1File {
    V1File {
        path: rel(path),
        sha256: knowledge_core::data::ContentDigest::new(format!("{:x}", Sha256::digest(bytes)))
            .unwrap(),
        size: bytes.len() as u64,
    }
}

/// Native test host supplies context in a separate real file, never a package DTO grant.
/// This proves binding to supplied context, not an actual external Notebook publication.
fn host(path: &Path, files: &[V1File], revision: &str) {
    let inventory:Vec<_>=files.iter().map(|f|json!({"path":f.path.as_str(),"contentDigest":format!("sha256:{}",f.sha256.as_str()),"byteLength":f.size})).collect();
    fs::write(path,serde_json::to_vec_pretty(&json!({"schemaVersion":1,"format":"openkit.knowledge-package","ownerScope":{"kind":"server"},"knowledgeBaseId":"kb_019aa000-0000-7000-8000-000000000001","notebookRevision":revision,"requiredFeatures":["openkit.knowledge-git-notebook.v1","openkit.knowledge-owner-scope.v1"],"mounts":[{"name":"local","backend":"local","root":"raw/","default":true},{"name":"offline","backend":"nas","root":"/never-contact","default":false}],"contentInventory":inventory,"hostExtension":{"retained":true}})).unwrap()).unwrap();
}

/// Write real ordinary files and a genuine-format manifest; no fake backend or filesystem is used.
fn directory(root: &Path) -> Vec<V1File> {
    fs::create_dir_all(root.join("pages")).unwrap();
    fs::create_dir_all(root.join("raw")).unwrap();
    fs::create_dir_all(root.join("schema")).unwrap();
    let entries = [
        (
            "pages/a.md",
            b"# Page\r\n[asset](../raw/a.bin#piece)\r\n`[example](../raw/a.bin)`\r\n".as_slice(),
        ),
        ("raw/a.bin", b"\0\xffraw\r\n".as_slice()),
        (
            "schema/scope-schema.yaml",
            b"# Inert\nreview: approved\n".as_slice(),
        ),
    ];
    let mut files = Vec::new();
    for (path, bytes) in entries {
        fs::write(root.join(path), bytes).unwrap();
        files.push(selected(path, bytes));
    }
    host(
        &root.join("manifest.json"),
        &files,
        "supplied-original-publication",
    );
    files
}

/// Independent tar crate produces genuine ustar/PAX input, including physical directory headers.
fn carrier(input: &Path, output: &Path, paths: &[&str], pax: Option<(&str, &str)>) {
    let mut archive = tar::Builder::new(File::create(output).unwrap());
    let mut h = tar::Header::new_ustar();
    h.set_entry_type(tar::EntryType::Directory);
    h.set_path("pages/").unwrap();
    h.set_mode(0o700);
    h.set_size(0);
    h.set_cksum();
    archive.append(&h, &[][..]).unwrap();
    for path in paths {
        let bytes = fs::read(input.join(path)).unwrap();
        if let Some((name, value)) = pax
            && *path == "raw/long.bin"
        {
            archive
                .append_pax_extensions([(name, value.as_bytes())])
                .unwrap();
        }
        let mut h = tar::Header::new_ustar();
        h.set_path(path).unwrap();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o600);
        h.set_cksum();
        archive.append(&h, bytes.as_slice()).unwrap();
    }
    archive.finish().unwrap();
}

/// Verify reopening and independent unpacked member identities without extracting into any home.
#[test]
fn genuine_tar_round_trip_uses_held_ranges_and_real_native_output() {
    let root = dir("phase2-tar");
    let input = root.join("input");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    directory(&input);
    let tar = root.join("input.tar");
    carrier(
        &input,
        &tar,
        &[
            "manifest.json",
            "pages/a.md",
            "raw/a.bin",
            "schema/scope-schema.yaml",
        ],
        None,
    );
    let output = root.join("output.tar");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let mut package = V1Package::open_tar(File::open(&tar).unwrap(), work).unwrap();
        assert_eq!(package.files().len(), 3);
        package
            .with_file(&rel("raw/a.bin"), |source, work| {
                let mut copy = File::create(root.join("copy.bin")).unwrap();
                source.copy_to(&mut copy, work)
            })
            .unwrap();
        assert_eq!(
            fs::read(root.join("copy.bin")).unwrap(),
            fs::read(input.join("raw/a.bin")).unwrap()
        );
        let artifact = package
            .reexport_tar(&private, "roundtrip", &output)
            .unwrap();
        assert_eq!(artifact.files, 3);
        let reopened = V1Package::open_tar(File::open(&output).unwrap(), work).unwrap();
        assert_eq!(reopened.context()["hostExtension"]["retained"], true);
        drop(reopened);
        let mut pinned = V1Package::open_tar(File::open(&tar).unwrap(), work).unwrap();
        fs::rename(&tar, root.join("held-original.tar")).unwrap();
        fs::write(&tar, b"replacement is not this source").unwrap();
        let result = pinned.with_file(&rel("raw/a.bin"), |source, work| {
            let mut copy = File::create(root.join("copy.bin")).unwrap();
            source.copy_to(&mut copy, work)
        });
        // Rename can change Unix ctime; refusing is safe, redirecting to the replacement is not.
        assert!(result == Ok(()) || result == Err(Error::Drift));
        assert_eq!(
            fs::read(root.join("copy.bin")).unwrap(),
            fs::read(input.join("raw/a.bin")).unwrap()
        );
    });
    drop(held);
    let mut independent = tar::Archive::new(File::open(&output).unwrap());
    for member in independent.entries().unwrap() {
        let mut member = member.unwrap();
        let path = member.path().unwrap().into_owned();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, fs::read(input.join(path)).unwrap());
    }
    assert_eq!(fs::read(&tar).unwrap(), b"replacement is not this source");
    fs::remove_dir_all(root).unwrap();
}

/// Local links and parsed Source scalars change; code/prose/labels/raw pins and extensions do not.
#[test]
fn native_nonidentity_projection_and_host_bound_directory_and_tar_export() {
    let root = dir("phase2-project");
    let input = root.join("native");
    let private = root.join("private");
    for path in [
        input.join("wiki"),
        input.join("raw"),
        input.join("sources"),
        input.join("schema"),
        input.join("originals"),
        private.clone(),
    ] {
        fs::create_dir_all(path).unwrap();
    }
    let raw = b"\0\xfforiginal\r\n";
    let digest = format!("sha256:{:x}", Sha256::digest(raw));
    let from_ref = format!("source:{OLD}@{digest}");
    let to_ref = format!("source:{NEW}@{digest}");
    let page = format!(
        "---\r\nannotation: '[not-link](../raw/original.bin)'\r\nrefs:\r\n  - '{from_ref}' # pin\r\n---\r\nProse ../raw/original.bin.\r\n`[code](../raw/original.bin)`\r\n```md\r\n[code](../raw/original.bin)\r\n```\r\n[label](../raw/original.bin#part) ![image](<../raw/original.bin> \"title\")\r\n[asset][ref]\r\n\r\n[ref]: ../raw/original.bin \"keep\"\r\n\r\n[source]({from_ref})\r\n<a href=\"../raw/original.bin\">HTML stays</a>\r\n![alt [display](../raw/original.bin)](../raw/original.bin)\r\n[orphan](../raw/unavailable.bin) [self](?view=1) [directory](../raw/)\r\n"
    );
    let pointer = format!(
        "---\r\ncontentRef:\r\n  locator:\r\n    mount: local\r\n    path: 'original.bin' # actual asset\r\n  contentDigest: {digest}\r\nannotation: original.bin\r\nreview_state: approved\r\n---\r\nOrdinary authored annotation.\r\n"
    );
    let files = [
        selected("wiki/a.md", page.as_bytes()),
        selected("raw/original.bin", raw),
        selected(&format!("sources/{OLD}.md"), pointer.as_bytes()),
        selected(
            "schema/scope-schema.yaml",
            b"version: 1\n# Authored spacing stays unchanged.\npageTypes: {}\n",
        ),
        selected(
            "originals/readme.md",
            b"[source bytes](../raw/original.bin)\r\n",
        ),
    ];
    for (file, bytes) in files.iter().zip([
        page.as_bytes(),
        raw.as_slice(),
        pointer.as_bytes(),
        b"version: 1\n# Authored spacing stays unchanged.\npageTypes: {}\n".as_slice(),
        b"[source bytes](../raw/original.bin)\r\n".as_slice(),
    ]) {
        fs::write(input.join(file.path.as_str()), bytes).unwrap();
    }
    let mappings = [
        V1PathMap {
            from: rel("wiki/a.md"),
            to: rel("pages/nested/a.md"),
        },
        V1PathMap {
            from: rel("raw/original.bin"),
            to: rel("raw/moved.bin"),
        },
        V1PathMap {
            from: rel(&format!("sources/{OLD}.md")),
            to: rel(&format!("sources/{NEW}.md")),
        },
        V1PathMap {
            from: rel("originals/readme.md"),
            to: rel("raw/readme.md"),
        },
    ];
    let references = [V1ReferenceMap {
        from: from_ref.clone(),
        to: to_ref.clone(),
    }];
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let mut projection =
            V1Projection::from_native(&input, &files, &mappings, &references, Some("local"), work)
                .unwrap();
        assert_eq!(
            projection.files()[1].source.sha256,
            projection.files()[1].projected.sha256
        );
        assert_eq!(
            projection
                .export(
                    None,
                    V1Carrier::Directory,
                    &root.join("must-not-open"),
                    "refused",
                    &root.join("absent")
                )
                .unwrap(),
            V1ExportOutcome::CompatibilityRefused
        );
        assert!(!root.join("must-not-open").exists());
        let supplied = root.join("supplied-host.json");
        let projected: Vec<_> = projection
            .files()
            .iter()
            .map(|f| f.projected.clone())
            .collect();
        host(&supplied, &projected, "supplied-finalized-publication");
        for (carrier, name) in [(V1Carrier::Directory, "directory"), (V1Carrier::Tar, "tar")] {
            let output = root.join(name);
            let result = projection
                .export(
                    Some(File::open(&supplied).unwrap()),
                    carrier,
                    &private,
                    name,
                    &output,
                )
                .unwrap();
            assert!(matches!(result, V1ExportOutcome::Published(_)));
        }
        drop(projection);
        let output = root.join("directory");
        let actual = fs::read_to_string(output.join("pages/nested/a.md")).unwrap();
        assert!(actual.contains(
            "[label](../../raw/moved.bin#part) ![image](<../../raw/moved.bin> \"title\")"
        ));
        assert!(actual.contains("[ref]: ../../raw/moved.bin \"keep\""));
        assert!(actual.contains(&format!("[source]({to_ref})")));
        assert!(actual.contains(&format!("  - '{to_ref}' # pin\r\n")));
        assert!(actual.contains("annotation: '[not-link](../raw/original.bin)'\r\n"));
        assert!(actual.contains("Prose ../raw/original.bin.\r\n"));
        assert!(actual.contains("`[code](../raw/original.bin)`\r\n"));
        assert!(actual.contains("```md\r\n[code](../raw/original.bin)\r\n```"));
        assert!(actual.contains("<a href=\"../raw/original.bin\">HTML stays</a>"));
        assert!(actual.contains("![alt [display](../raw/original.bin)](../../raw/moved.bin)"));
        assert!(actual.contains(
            "[orphan](../../raw/unavailable.bin) [self](?view=1) [directory](../../raw/)"
        ));
        assert!(!output.join("raw/unavailable.bin").exists());
        let source = fs::read_to_string(output.join(format!("sources/{NEW}.md"))).unwrap();
        assert_eq!(
            source,
            pointer.replace("path: 'original.bin'", "path: 'moved.bin'")
        );
        assert!(source.contains(&digest));
        assert_eq!(fs::read(output.join("raw/moved.bin")).unwrap(), raw);
        assert_eq!(
            fs::read(output.join("raw/readme.md")).unwrap(),
            b"[source bytes](../raw/original.bin)\r\n"
        );
        let reopened = V1Package::open(&output, work).unwrap();
        assert_eq!(
            reopened.context()["notebookRevision"],
            "supplied-finalized-publication"
        );
        drop(reopened);
        let reopened = V1Package::open_tar(File::open(root.join("tar")).unwrap(), work).unwrap();
        assert_eq!(reopened.files().len(), 5);
        drop(reopened);
    });
    drop(held);
    assert_eq!(fs::read(input.join("wiki/a.md")).unwrap(), page.as_bytes());
    fs::remove_dir_all(root).unwrap();
}

/// The original foreign revision cannot finalize changed paths/bytes; incorrect inventory also refuses.
#[test]
fn changed_foreign_context_and_wrong_host_inventory_refuse_before_output() {
    let root = dir("phase2-context");
    let input = root.join("input");
    directory(&input);
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let package = V1Package::open(&input, work).unwrap();
        let mut projection = package
            .project(
                &[V1PathMap {
                    from: rel("raw/a.bin"),
                    to: rel("raw/new.bin"),
                }],
                &[],
            )
            .unwrap();
        let files: Vec<_> = projection
            .files()
            .iter()
            .map(|f| f.projected.clone())
            .collect();
        let context = root.join("host.json");
        host(&context, &files, "supplied-original-publication");
        assert_eq!(
            projection.export(
                Some(File::open(&context).unwrap()),
                V1Carrier::Tar,
                &private,
                "stale",
                &root.join("output")
            ),
            Err(Error::CompatibilityRefused)
        );
        assert_eq!(fs::read_dir(&private).unwrap().count(), 0);
        host(&context, &files, "new-supplied-publication");
        let mut wrong: Value = serde_json::from_slice(&fs::read(&context).unwrap()).unwrap();
        wrong["contentInventory"][0]["contentDigest"] = json!(format!("sha256:{}", "0".repeat(64)));
        fs::write(&context, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert_eq!(
            projection.export(
                Some(File::open(&context).unwrap()),
                V1Carrier::Directory,
                &private,
                "wrong",
                &root.join("output")
            ),
            Err(Error::Checksum)
        );
        assert_eq!(fs::read_dir(&private).unwrap().count(), 0);
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Native POSIX/PAX long names are accepted without a new path ordering or reduced name ceiling.
#[test]
fn genuine_pax_long_path_reexports_and_reopens() {
    let root = dir("phase2-pax");
    let input = root.join("input");
    let mut files = directory(&input);
    let long = format!("raw/{}.bin", "x".repeat(160));
    let bytes = b"long ordinary asset";
    fs::write(input.join("raw/long.bin"), bytes).unwrap();
    fs::remove_file(input.join("raw/a.bin")).unwrap();
    files.retain(|f| f.path.as_str() != "raw/a.bin");
    files.push(selected(&long, bytes));
    host(
        &input.join("manifest.json"),
        &files,
        "supplied-long-publication",
    );
    let archive = root.join("long.tar");
    carrier(
        &input,
        &archive,
        &[
            "manifest.json",
            "pages/a.md",
            "raw/long.bin",
            "schema/scope-schema.yaml",
        ],
        Some(("path", &long)),
    );
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let output = root.join("output.tar");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let package = V1Package::open_tar(File::open(&archive).unwrap(), work).unwrap();
        assert!(package.files().iter().any(|f| f.path.as_str() == long));
        package.reexport_tar(&private, "long", &output).unwrap();
        let again = V1Package::open_tar(File::open(&output).unwrap(), work).unwrap();
        assert!(again.files().iter().any(|f| f.path.as_str() == long));
    });
    drop(held);
    let mut reader = tar::Archive::new(File::open(&output).unwrap());
    assert!(
        reader
            .entries()
            .unwrap()
            .any(|e| e.unwrap().path().unwrap().as_ref() == Path::new(&long))
    );
    fs::remove_dir_all(root).unwrap();
}

/// Framing, special/link/PAX semantics, duplicates, byte corruption and missing inventory fail inertly.
#[test]
fn hostile_tar_controls_leave_originals_and_unrelated_paths_unchanged() {
    let root = dir("phase2-hostile");
    let input = root.join("input");
    directory(&input);
    let marker = root.join("outside");
    fs::write(&marker, b"untouched").unwrap();
    let good = root.join("good.tar");
    carrier(
        &input,
        &good,
        &[
            "manifest.json",
            "pages/a.md",
            "raw/a.bin",
            "schema/scope-schema.yaml",
        ],
        None,
    );
    let original = fs::read(&good).unwrap();
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    for case in [
        "checksum",
        "truncated",
        "trailing",
        "digest",
        "duplicate",
        "symlink",
        "sparse",
        "missing",
    ] {
        let archive = root.join(format!("{case}.tar"));
        match case {
            "checksum" => {
                let mut bytes = original.clone();
                bytes[12] ^= 1;
                fs::write(&archive, bytes).unwrap();
            }
            "truncated" => fs::write(&archive, &original[..original.len() - 1]).unwrap(),
            "trailing" => {
                let mut bytes = original.clone();
                bytes.extend_from_slice(&[1; 512]);
                fs::write(&archive, bytes).unwrap();
            }
            "digest" => {
                let mut bytes = original.clone();
                let at = bytes
                    .windows(7)
                    .position(|b| b == b"\0\xffraw\r\n")
                    .unwrap();
                bytes[at + 2] ^= 1;
                fs::write(&archive, bytes).unwrap();
            }
            "duplicate" => carrier(
                &input,
                &archive,
                &[
                    "manifest.json",
                    "pages/a.md",
                    "pages/a.md",
                    "raw/a.bin",
                    "schema/scope-schema.yaml",
                ],
                None,
            ),
            "missing" => carrier(
                &input,
                &archive,
                &["manifest.json", "pages/a.md", "schema/scope-schema.yaml"],
                None,
            ),
            "symlink" | "sparse" => {
                let mut builder = tar::Builder::new(File::create(&archive).unwrap());
                if case == "sparse" {
                    builder
                        .append_pax_extensions([("GNU.sparse.size", b"7".as_slice())])
                        .unwrap();
                }
                let mut header = tar::Header::new_ustar();
                header.set_path("raw/a.bin").unwrap();
                header.set_mode(0o600);
                header.set_size(0);
                if case == "symlink" {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_link_name("../../outside").unwrap();
                }
                header.set_cksum();
                builder.append(&header, &[][..]).unwrap();
                builder.finish().unwrap();
            }
            _ => unreachable!(),
        }
        let held = run(&limits, &cancel, |work| {
            assert!(
                V1Package::open_tar(File::open(&archive).unwrap(), work).is_err(),
                "{case}"
            );
        });
        drop(held);
        assert_eq!(fs::read(&marker).unwrap(), b"untouched");
    }
    assert_eq!(fs::read(&good).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

/// Existing output and interrupted evidence survive; a changed source cannot use a stale projection.
#[test]
fn projected_publication_preserves_conflicts_and_refuses_source_drift() {
    let root = dir("phase2-effects");
    let input = root.join("input");
    directory(&input);
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let existing = root.join("existing.tar");
    fs::write(&existing, b"previous output").unwrap();
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let package = V1Package::open(&input, work).unwrap();
        let mut projection = package.project(&[], &[]).unwrap();
        assert_eq!(
            projection.export(
                Some(File::open(input.join("manifest.json")).unwrap()),
                V1Carrier::Tar,
                &private,
                "race",
                &existing
            ),
            Err(Error::Conflict)
        );
        assert_eq!(fs::read(&existing).unwrap(), b"previous output");
        assert!(private.join("race/package.tar").exists());
        assert_eq!(
            projection.export(
                Some(File::open(input.join("manifest.json")).unwrap()),
                V1Carrier::Tar,
                &private,
                "race",
                &existing
            ),
            Err(Error::Interrupted)
        );
        fs::write(input.join("raw/a.bin"), b"changed").unwrap();
        let mut writer = File::create(root.join("owner-private")).unwrap();
        assert!(
            projection
                .copy_file(&rel("raw/a.bin"), &mut writer)
                .is_err()
        );
        assert_eq!(fs::metadata(root.join("owner-private")).unwrap().len(), 0);
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// A real raw payload larger than admitted memory streams without extraction or a whole-payload buffer.
#[test]
fn finalized_native_tar_streams_payload_larger_than_memory_budget() {
    let root = dir("phase2-stream");
    let input = root.join("native");
    let private = root.join("private");
    for path in [input.join("raw"), input.join("schema"), private.clone()] {
        fs::create_dir_all(path).unwrap();
    }
    let bytes = vec![0xacu8; 1_500_000];
    fs::write(input.join("raw/asset.bin"), &bytes).unwrap();
    let schema = b"version: 1\npageTypes: {}\n";
    fs::write(input.join("schema/scope-schema.yaml"), schema).unwrap();
    let files = [
        selected("raw/asset.bin", &bytes),
        selected("schema/scope-schema.yaml", schema),
    ];
    let context = root.join("host.json");
    host(&context, &files, "supplied-stream-publication");
    let mut limits = bounds(8_000_000);
    limits.budget.max_memory_bytes = 1_000_000;
    let cancel = AtomicBool::new(false);
    let output = root.join("output.tar");
    let held = run(&limits, &cancel, |work| {
        let mut projection =
            V1Projection::from_native(&input, &files, &[], &[], Some("local"), work).unwrap();
        let result = projection
            .export(
                Some(File::open(&context).unwrap()),
                V1Carrier::Tar,
                &private,
                "stream",
                &output,
            )
            .unwrap();
        assert!(matches!(result, V1ExportOutcome::Published(_)));
    });
    drop(held);
    assert!(fs::metadata(&output).unwrap().len() > limits.budget.max_memory_bytes);
    let mut independent = tar::Archive::new(File::open(&output).unwrap());
    for entry in independent.entries().unwrap() {
        let mut entry = entry.unwrap();
        if entry.path().unwrap().as_ref() == Path::new("raw/asset.bin") {
            let mut actual = Vec::new();
            entry.read_to_end(&mut actual).unwrap();
            assert_eq!(actual, bytes);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

/// Admission precedes native payload reads; unsupported mapped YAML and digest retargeting never publish.
#[test]
fn tar_control_and_mapping_refusals_preserve_native_inputs() {
    let root = dir("phase2-refusals");
    let input = root.join("input");
    directory(&input);
    let archive = root.join("input.tar");
    carrier(
        &input,
        &archive,
        &[
            "manifest.json",
            "pages/a.md",
            "raw/a.bin",
            "schema/scope-schema.yaml",
        ],
        None,
    );
    let mut limits = bounds(8_000_000);
    limits.budget.max_bytes = 1;
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        assert!(matches!(
            V1Package::open_tar(File::open(&archive).unwrap(), work),
            Err(Error::Quota)
        ));
        assert_eq!(work.read_bytes, 0);
    });
    drop(held);
    let limits = bounds(8_000_000);
    for case in ["alias", "flow", "pin"] {
        let native = root.join(case);
        fs::create_dir_all(native.join("raw")).unwrap();
        fs::create_dir_all(native.join("sources")).unwrap();
        let raw = b"raw bytes";
        fs::write(native.join("raw/a.bin"), raw).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(raw));
        let pointer = if case == "alias" {
            format!(
                "---\nlabel: Don't guess quotes\ncontentRef: &pointer\n  locator:\n    mount: local\n    path: a.bin\n  contentDigest: {digest}\ncopy: *pointer\n---\nAnnotation\n"
            )
        } else if case == "flow" {
            format!(
                "---\ncontentRef: {{locator: {{mount: local, path: a.bin}}, contentDigest: '{digest}'}}\n---\nAnnotation\n"
            )
        } else {
            format!(
                "---\ncontentRef:\n  locator:\n    mount: local\n    path: a.bin\n  contentDigest: {digest}\n---\nAnnotation\n"
            )
        };
        let path = format!("sources/{OLD}.md");
        fs::write(native.join(&path), &pointer).unwrap();
        let files = [
            selected("raw/a.bin", raw),
            selected(&path, pointer.as_bytes()),
        ];
        let mappings = [V1PathMap {
            from: rel("raw/a.bin"),
            to: rel("raw/b.bin"),
        }];
        let refs = if case == "pin" {
            vec![V1ReferenceMap {
                from: format!("source:{OLD}@{digest}"),
                to: format!("source:{NEW}@sha256:{}", "0".repeat(64)),
            }]
        } else {
            Vec::new()
        };
        let held = run(&limits, &cancel, |work| {
            let result =
                V1Projection::from_native(&native, &files, &mappings, &refs, Some("local"), work);
            assert!(
                matches!(result, Err(Error::Unsupported)) && case != "pin"
                    || matches!(result, Err(Error::Invalid)) && case == "pin"
            );
        });
        drop(held);
        assert_eq!(fs::read(native.join(&path)).unwrap(), pointer.as_bytes());
        assert_eq!(fs::read(native.join("raw/a.bin")).unwrap(), raw);
    }
    fs::remove_dir_all(root).unwrap();
}

/// The native owner sees a verified read-only selected-payload FD, never an original archive descriptor.
/// A post-callback cancellation preserves the actual returned native FD instead of hiding owner state.
#[test]
fn native_owner_handoff_retains_real_result_and_post_callback_control() {
    let root = dir("phase2-native-owner");
    let input = root.join("input");
    directory(&input);
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let limits = bounds(8_000_000);
    for cancelled in [false, true] {
        let cancel = AtomicBool::new(false);
        let held = run(&limits, &cancel, |work| {
            let mut package = V1Package::open(&input, work).unwrap();
            package
                .with_file(&rel("raw/a.bin"), |source, work| {
                    let operation = if cancelled { "cancelled" } else { "native" };
                    let handoff = source.with_private_file(
                        &private,
                        operation,
                        work,
                        |file, digest, work| {
                            assert!(work.holding.remaining().is_ok());
                            assert_eq!(digest, &source_digest());
                            let mut native = file.try_clone().unwrap();
                            assert!(native.write_all(b"cannot mutate selected input").is_err());
                            if cancelled {
                                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            native
                        },
                    )?;
                    assert_eq!(
                        handoff.source_revalidation,
                        if cancelled {
                            Err(Error::Cancelled)
                        } else {
                            Ok(())
                        }
                    );
                    let mut actual = Vec::new();
                    let mut returned = handoff.owner;
                    returned.read_to_end(&mut actual).unwrap();
                    assert_eq!(actual, b"\0\xffraw\r\n");
                    assert!(work.holding.remaining().is_ok());
                    if !cancelled {
                        assert!(matches!(
                            source.with_private_file(&private, operation, work, |_, _, _| panic!(
                                "replayed owner"
                            )),
                            Err(Error::Interrupted)
                        ));
                    }
                    Ok(())
                })
                .unwrap();
        });
        drop(held);
    }
    assert_eq!(fs::read(input.join("raw/a.bin")).unwrap(), b"\0\xffraw\r\n");
    fs::remove_dir_all(root).unwrap();
}

/// A replaced private directory entry cannot redirect the held descriptor, and a changed source stays visible.
/// An existing operation directory is left in place with the original source.
#[test]
fn held_descriptor_ignores_path_replacement_and_keeps_conflict() {
    let root = dir("phase2-held-fd");
    let input = root.join("input");
    directory(&input);
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    fs::create_dir(private.join("exists")).unwrap();
    fs::write(private.join("exists/keep"), b"original-destination").unwrap();
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |work| {
        let mut package = V1Package::open(&input, work).unwrap();
        package
            .with_file(&rel("raw/a.bin"), |source, work| {
                assert!(matches!(
                    source.with_private_file(&private, "exists", work, |_, _, _| ()),
                    Err(Error::Interrupted)
                ));
                let stage = private.join("held");
                let handoff =
                    source.with_private_file(&private, "held", work, |file, digest, _| {
                        use std::os::unix::fs::{FileExt, MetadataExt};
                        let mut buffer = [0u8; 16];
                        let read = file.read_at(&mut buffer, 0).unwrap();
                        assert_eq!(&buffer[..read], b"\0\xffraw\r\n");
                        let inode = file.metadata().unwrap().ino();
                        fs::rename(&stage, private.join("held-displaced")).unwrap();
                        fs::create_dir(&stage).unwrap();
                        fs::write(stage.join("source"), b"replaced-path").unwrap();
                        let read = file.read_at(&mut buffer, 0).unwrap();
                        assert_eq!(&buffer[..read], b"\0\xffraw\r\n");
                        assert_eq!(file.metadata().unwrap().ino(), inode);
                        let mut clone = file.try_clone().unwrap();
                        assert!(std::io::Write::write_all(&mut clone, b"x").is_err());
                        fs::write(input.join("raw/a.bin"), b"stale-source").unwrap();
                        digest.clone()
                    })?;
                assert_eq!(handoff.source_revalidation, Err(Error::Drift));
                assert_eq!(handoff.owner, source_digest());
                Ok(())
            })
            .unwrap();
    });
    drop(held);
    assert_eq!(
        fs::read(private.join("exists/keep")).unwrap(),
        b"original-destination"
    );
    assert_eq!(fs::read(input.join("raw/a.bin")).unwrap(), b"stale-source");
    assert_eq!(
        fs::read(private.join("held-displaced/source")).unwrap(),
        b"\0\xffraw\r\n"
    );
    fs::remove_dir_all(root).unwrap();
}

/// Independently calculated fixture identity; no signer or Git success result is invented.
fn source_digest() -> knowledge_core::data::ContentDigest {
    selected("raw/a.bin", b"\0\xffraw\r\n").sha256
}
