//! Real retained-v1 directory fixtures exercise inventory, inert mapping, and unchanged export.
use super::super::*;
use knowledge_package::V1Directory;
use sha2::{Digest, Sha256};

/// Build ordinary files in the independently owned retained-v1 shape, not a backend substitute.
fn fixture(root: &Path) {
    fs::create_dir(root.join("pages")).unwrap();
    fs::create_dir(root.join("raw")).unwrap();
    fs::create_dir(root.join("schema")).unwrap();
    let files: [(&str, &[u8]); 3] = [
        (
            "pages/guide.md",
            b"# Page\r\n[raw](../raw/original.bin)\r\n`../raw/no-rewrite`\n",
        ),
        ("raw/original.bin", b"\0\xfforiginal\r\n"),
        (
            "schema/scope-schema.yaml",
            b"# Foreign labels remain inert\nreview: approved\n",
        ),
    ];
    let mut inventory = Vec::new();
    for (path, bytes) in files {
        fs::write(root.join(path), bytes).unwrap();
        inventory.push(serde_json::json!({"path": path, "contentDigest": format!("sha256:{:x}", Sha256::digest(bytes)), "byteLength": bytes.len()}));
    }
    let manifest = serde_json::json!({
        "schemaVersion": 1, "format": "openkit.knowledge-package",
        "ownerScope": {"kind": "server"},
        "knowledgeBaseId": "kb_019aa000-0000-7000-8000-000000000001",
        "notebookRevision": "0123456789abcdef0123456789abcdef01234567",
        "requiredFeatures": ["openkit.knowledge-git-notebook.v1", "openkit.knowledge-owner-scope.v1"],
        "contentInventory": inventory,
        "mounts": [{"name": "local", "backend": "local", "root": "raw/", "default": true},
                   {"name": "offline", "backend": "nas", "root": "/must-not-contact", "default": false}],
        "authoredExtension": {"review": "not a grant"}
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

/// Genuine input context can travel unchanged; raw/page/schema bytes and extensions survive.
#[test]
fn genuine_v1_mapping_and_context_preserving_reexport() {
    let root = dir("foreign-native");
    let original = root.join("original");
    let private = root.join("private");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&private).unwrap();
    fixture(&original);
    let dest = root.join("exported");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let mut foreign = V1Directory::open(&original, job).unwrap();
        assert_eq!(foreign.files().len(), 3);
        assert_eq!(
            foreign.context()["authoredExtension"]["review"],
            "not a grant"
        );
        assert_eq!(foreign.context()["ownerScope"]["kind"], "server");
        foreign.reexport(&private, "export1", &dest).unwrap();
        drop(foreign);
        let reopened = V1Directory::open(&dest, job).unwrap();
        assert_eq!(reopened.files().len(), 3);
        drop(reopened);
        for path in [
            "manifest.json",
            "pages/guide.md",
            "raw/original.bin",
            "schema/scope-schema.yaml",
        ] {
            assert_eq!(
                fs::read(original.join(path)).unwrap(),
                fs::read(dest.join(path)).unwrap()
            );
        }
        assert!(!root.join("must-not-contact").exists());
        assert!(!dest.join(".git").exists());
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Changed content cannot retain a false Notebook publication during export.
#[test]
fn changed_v1_bytes_refuse_without_destination_effect() {
    let root = dir("foreign-changed");
    let original = root.join("original");
    let private = root.join("private");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&private).unwrap();
    fixture(&original);
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let dest = root.join("exported");
    let held = run(&limits, &cancel, |job| {
        let mut foreign = V1Directory::open(&original, job).unwrap();
        fs::write(original.join("pages/guide.md"), b"changed").unwrap();
        assert!(foreign.reexport(&private, "changed", &dest).is_err());
        assert!(!dest.exists());
        assert!(
            private.join("changed").exists(),
            "private failure evidence is retained"
        );
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Links, undeclared bytes, unknown required semantics, and reserved empty mount folders refuse.
#[test]
fn v1_closure_and_required_semantics_are_not_bypassed() {
    let root = dir("foreign-controls");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    for case in [
        "undeclared",
        "hardlink",
        "symlink",
        "reserved",
        "feature",
        "digest",
    ] {
        let path = root.join(case);
        fs::create_dir(&path).unwrap();
        fixture(&path);
        match case {
            "undeclared" => fs::write(path.join("raw/extra.bin"), b"not inventoried").unwrap(),
            "hardlink" => {
                fs::hard_link(path.join("raw/original.bin"), root.join("linked.bin")).unwrap()
            }
            "symlink" => {
                fs::remove_file(path.join("raw/original.bin")).unwrap();
                std::os::unix::fs::symlink("../../outside", path.join("raw/original.bin")).unwrap();
            }
            "reserved" => fs::create_dir(path.join("raw/offline")).unwrap(),
            "feature" => {
                let file = path.join("manifest.json");
                let mut v: serde_json::Value =
                    serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
                v["requiredFeatures"] = serde_json::json!(["unknown.required"]);
                fs::write(file, serde_json::to_vec(&v).unwrap()).unwrap();
            }
            "digest" => fs::write(path.join("raw/original.bin"), b"same length?").unwrap(),
            _ => unreachable!(),
        }
        let held = run(&limits, &cancel, |job| {
            assert!(V1Directory::open(&path, job).is_err(), "{case}");
        });
        drop(held);
    }
    fs::remove_dir_all(root).unwrap();
}

/// Atomic directory no-clobber leaves even an empty competing destination untouched.
#[test]
fn v1_export_preserves_destination_race_and_private_evidence() {
    let root = dir("foreign-race");
    let original = root.join("original");
    let private = root.join("private");
    let dest = root.join("existing");
    for p in [&original, &private, &dest] {
        fs::create_dir(p).unwrap();
    }
    fixture(&original);
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let mut foreign = V1Directory::open(&original, job).unwrap();
        assert_eq!(
            foreign.reexport(&private, "race", &dest),
            Err(Error::Conflict)
        );
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        assert!(private.join("race/manifest.json").exists());
        assert_eq!(
            foreign.reexport(&private, "race", &dest),
            Err(Error::Interrupted)
        );
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}
