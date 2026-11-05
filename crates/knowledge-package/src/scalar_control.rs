//! Projection controls for remapped YAML string scalars and retained references.
use crate::{Bounds, Quotas, V1File, V1PathMap, V1Projection, V1ReferenceMap, Work};
use knowledge_core::admission::{Account, Configuration, Decision, Holding, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::Reservation;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

const OLD: &str = "ks_019aa000-0000-7000-8000-000000000001";
const NEW: &str = "ks_019aa000-0000-7000-8000-000000000002";

struct OwnedDir(PathBuf);
impl Drop for OwnedDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Remapped locator filenames that are plain YAML null, bool, or number stay strings after parse.
/// Quoted and escaped forms round-trip. Unmoved orphan, self, and directory refs stay byte-identical.
#[test]
fn projection_keeps_string_locators_and_retained_refs() {
    let root = OwnedDir(temp_dir("scalar-locators"));
    let input = root.0.join("native");
    fs::create_dir_all(input.join("raw")).unwrap();
    fs::create_dir_all(input.join("sources")).unwrap();
    fs::create_dir_all(input.join("wiki")).unwrap();
    let cases = [
        ("kept.bin", Style::Plain),
        ("null", Style::Plain),
        ("true", Style::Plain),
        ("false", Style::Plain),
        ("01", Style::Plain),
        ("2.5", Style::Plain),
        ("1e2", Style::Plain),
        ("0x10", Style::Plain),
        ("3", Style::Plain),
        ("yes", Style::Plain),
        ("n", Style::Plain),
        ("on", Style::Plain),
        ("~", Style::Plain),
        (".inf", Style::Plain),
        ("say\"hi\".bin", Style::Plain),
        ("a #b.bin", Style::Plain),
        ("1", Style::Single),
        ("it's.bin", Style::Single),
        ("y", Style::Double),
    ];
    let mut files = Vec::new();
    let mut paths = Vec::new();
    let mut null_digest = String::new();
    for (index, (target, style)) in cases.iter().copied().enumerate() {
        let bytes = format!("bytes-{index}-{target}").into_bytes();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if target == "null" {
            null_digest = digest.clone();
        }
        let asset = format!("raw/asset-{index}.bin");
        let page = format!("sources/s{index}.md");
        fs::write(input.join(&asset), &bytes).unwrap();
        let pointer = pointer(
            style,
            &format!("asset-{index}.bin"),
            &digest,
            target == "null",
        );
        fs::write(input.join(&page), &pointer).unwrap();
        files.push(file(&asset, &bytes));
        files.push(file(&page, pointer.as_bytes()));
        paths.push(V1PathMap {
            from: rel(&asset),
            to: rel(&format!("raw/{target}")),
        });
    }
    let keep = "\
[orphan](../raw/unavailable.bin) [self](?view=1) [directory](../raw/)
Nonidentity prose stays.
";
    fs::write(input.join("wiki/keep.md"), keep).unwrap();
    files.push(file("wiki/keep.md", keep.as_bytes()));
    let identity = format!("source:{OLD}@sha256:{null_digest}");
    let orphan_ref = format!("source:{NEW}@sha256:{null_digest}");
    let references = [V1ReferenceMap {
        from: identity.clone(),
        to: identity.clone(),
    }];
    let limits = limits();
    let cancel = AtomicBool::new(false);
    let (mut session, account_dir) = start_account();
    let mut work = Work::new(&limits, &mut session, &cancel).unwrap();
    let mut projection = V1Projection::from_native(
        &input,
        &files,
        &paths,
        &references,
        Some("local"),
        &mut work,
    )
    .unwrap();
    for (index, (target, style)) in cases.iter().copied().enumerate() {
        let page = rel(&format!("sources/s{index}.md"));
        let mut rendered = Vec::new();
        projection.copy_file(&page, &mut rendered).unwrap();
        let text = String::from_utf8(rendered).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(front(&text)).unwrap();
        let locator = &parsed["contentRef"]["locator"]["path"];
        assert!(locator.is_string(), "{target}");
        assert_eq!(locator.as_str(), Some(target));
        assert!(text.contains("prose null true 1\n"));
        match style {
            Style::Single => {
                assert!(text.contains(&format!("path: '{styled}'", styled = single(target))))
            }
            Style::Plain if target == "kept.bin" => assert!(text.contains("path: kept.bin\n")),
            Style::Plain | Style::Double => {}
        }
        if target == "null" {
            assert!(text.contains(&format!("  - '{identity}'\n")));
            assert!(text.contains(&format!("  - '{orphan_ref}'\n")));
            let refs = parsed["refs"].as_sequence().unwrap();
            assert_eq!(refs[0].as_str(), Some(identity.as_str()));
            assert_eq!(refs[1].as_str(), Some(orphan_ref.as_str()));
        }
        let projected = projection
            .files()
            .iter()
            .find(|item| item.projected.path == page)
            .unwrap();
        assert_eq!(
            projected.projected.sha256.as_str(),
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
        let mut asset_out = Vec::new();
        let asset = rel(&format!("raw/{target}"));
        projection.copy_file(&asset, &mut asset_out).unwrap();
        assert_eq!(asset_out, format!("bytes-{index}-{target}").into_bytes());
    }
    let mut kept = Vec::new();
    projection
        .copy_file(&rel("wiki/keep.md"), &mut kept)
        .unwrap();
    assert_eq!(kept, keep.as_bytes());
    drop(projection);
    drop(work);
    session.finish().unwrap();
    drop(account_dir);
}

#[derive(Clone, Copy)]
enum Style {
    Plain,
    Single,
    Double,
}

fn single(value: &str) -> String {
    value.replace('\'', "''")
}

fn pointer(style: Style, source: &str, digest: &str, with_refs: bool) -> String {
    let path = match style {
        Style::Plain => source.to_owned(),
        Style::Single => format!("'{}'", single(source)),
        Style::Double => format!("\"{source}\""),
    };
    let refs = if with_refs {
        format!("refs:\n  - 'source:{OLD}@sha256:{digest}'\n  - 'source:{NEW}@sha256:{digest}'\n")
    } else {
        String::new()
    };
    format!(
        "---\ncontentRef:\n  locator:\n    mount: local\n    path: {path}\n  contentDigest: sha256:{digest}\n{refs}---\nprose null true 1\n"
    )
}

fn front(text: &str) -> &str {
    text.split_once("---\n")
        .unwrap()
        .1
        .split_once("\n---\n")
        .unwrap()
        .0
}

fn file(path: &str, bytes: &[u8]) -> V1File {
    V1File {
        path: rel(path),
        sha256: ContentDigest::new(format!("{:x}", Sha256::digest(bytes))).unwrap(),
        size: bytes.len() as u64,
    }
}

fn rel(value: &str) -> RelativePath {
    RelativePath::new(value).unwrap()
}

fn limits() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 1_000,
            max_bytes: 80_000_000,
            max_millis: 30_000,
            max_memory_bytes: 32_000_000,
        },
        quotas: Quotas {
            max_archive_bytes: 8_000_000,
            max_entries: 100,
            max_entry_expanded: 2_000_000,
            max_total_expanded: 8_000_000,
            max_ratio: 1_000,
            max_path_bytes: 256,
            max_depth: 8,
        },
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("pkg-{name}-{nanos}"));
    fs::create_dir(&path).unwrap();
    path
}

fn start_account() -> (Holding, OwnedDir) {
    let path = OwnedDir(temp_dir("scalar-account"));
    fs::set_permissions(&path.0, fs::Permissions::from_mode(0o700)).unwrap();
    let directory = File::open(&path.0).unwrap();
    Account::start(
        directory,
        Configuration {
            memory_bytes: Some(16 * 1024 * 1024),
            storage_bytes: Some(256 * 1024 * 1024),
        },
        Scheduling {
            max_pending: 8,
            max_records: 256,
            max_descriptor_bytes: 1 << 20,
            per_mount: 4,
        },
    )
    .unwrap();
    let account = Account::current().unwrap();
    match account
        .reserve(
            Reservation {
                memory_bytes: 1,
                temporary_bytes: 1,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(holding) => (holding, path),
        Decision::Waiting(_) => panic!("account waiting"),
    }
}
