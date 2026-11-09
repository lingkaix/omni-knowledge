//! Built CLI initialization over real isolated cache and temporary filesystem locations.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Record every synthetic source entry and its bytes without treating access times as writes.
fn source_tree(root: &Path) -> Vec<(PathBuf, u32, Vec<u8>)> {
    /// Include directories so an empty newly created cache also changes the snapshot.
    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, u32, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        entries.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.mode(),
            if metadata.is_file() {
                fs::read(path).unwrap()
            } else {
                Vec::new()
            },
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort();
    entries
}

/// Exercise only the public SDK constructor in a fresh process with no prior Core owner.
fn sdk_startup_child() -> bool {
    let Some(root) = std::env::var_os("OMNI_INIT_SDK_ROOT") else {
        return false;
    };
    use knowledge_core::admission::Account;
    use omni_knowledge::KnowledgeBase;
    use omni_knowledge::native_service::NativeLibraryService;
    use omni_service as wire;
    use omni_service::authority::LocalPolicy;

    let root = PathBuf::from(root);
    let before = source_tree(&root);
    assert!(Account::current().is_err());
    let kb = KnowledgeBase::open(&root).unwrap();
    let result = NativeLibraryService::ordinary_local(
        kb,
        LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        },
    );
    let cache = PathBuf::from(std::env::var_os("OMNI_INIT_SDK_EXPECTED_CACHE").unwrap());
    if std::env::var_os("OMNI_INIT_SDK_REFUSE").is_some() {
        let error = match result {
            Ok(_) => panic!("SDK accepted an aliased cache inside its selected source home"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("outside the authored library"));
        assert!(Account::current().is_err());
        assert!(!cache.exists(), "refusal must precede cache creation");
    } else {
        let (_service, _owner, _selection) = result.unwrap();
        let held = Account::current()
            .unwrap()
            .generated_directory()
            .unwrap()
            .metadata()
            .unwrap();
        let selected = fs::metadata(cache).unwrap();
        assert_eq!((held.dev(), held.ino()), (selected.dev(), selected.ino()));
        eprintln!(
            "SDK source device={}, selected OS cache device={}",
            fs::metadata(&root).unwrap().dev(),
            held.dev()
        );
    }
    assert_eq!(
        source_tree(&root),
        before,
        "SDK startup wrote to its source"
    );
    true
}

/// Fresh initialization must index and remain searchable when TMPDIR is on another volume.
/// Storage owns one prepaid generated volume; selecting a temporary directory is not target binding.
#[test]
fn fresh_cli_init_selects_the_actual_publication_volume() {
    if sdk_startup_child() {
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let temporary = tempfile::tempdir_in("/tmp").unwrap();
    eprintln!(
        "synthetic init devices: home={}, temporary={}",
        fs::metadata(home.path()).unwrap().dev(),
        fs::metadata(temporary.path()).unwrap().dev()
    );
    let cache = home.path().join("os-cache");
    fs::create_dir(&cache).unwrap();
    let fake_home = home.path().join("process-home");
    fs::create_dir(&fake_home).unwrap();
    for root in [
        home.path().join("library"),
        temporary.path().join("library"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_omni"))
            .args(["init"])
            .arg(&root)
            .args(["--name", "Synthetic publication volume"])
            .env_remove("OMNI_CACHE_DIR")
            .env("XDG_CACHE_HOME", &cache)
            .env("HOME", &fake_home)
            .env("TMPDIR", temporary.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh init: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(root.join("omni-knowledge.json").is_file());
        assert!(
            root.join(".omni/current.json").is_file(),
            "init must retain its successful indexing behavior"
        );
        let output = Command::new(env!("CARGO_BIN_EXE_omni"))
            .arg("--root")
            .arg(&root)
            .args(["search", "knowledge", "--mode", "files", "--json"])
            .env_remove("OMNI_CACHE_DIR")
            .env("XDG_CACHE_HOME", &cache)
            .env("HOME", &fake_home)
            .env("TMPDIR", temporary.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "search initialized home: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("knowledge"));
        let output = Command::new(env!("CARGO_BIN_EXE_omni"))
            .arg("--root")
            .arg(&root)
            .args(["search", "knowledge", "--mode", "bm25", "--json"])
            .env_remove("OMNI_CACHE_DIR")
            .env("XDG_CACHE_HOME", &cache)
            .env("HOME", &fake_home)
            .env("TMPDIR", temporary.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "cold BM25 over initialized home: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let hits: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            hits.as_array().unwrap().iter().any(|hit| {
                hit["mode"] == "bm25"
                    && hit["path"]
                        .as_str()
                        .is_some_and(|path| root.join(path).is_file())
            }),
            "cold BM25 must return actual initialized source evidence on each selected volume"
        );
        #[cfg(target_os = "macos")]
        let os_cache = fake_home.join("Library/Caches");
        #[cfg(not(target_os = "macos"))]
        let os_cache = cache.clone();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "fresh_cli_init_selects_the_actual_publication_volume",
                "--nocapture",
            ])
            .env("OMNI_INIT_SDK_ROOT", &root)
            .env(
                "OMNI_INIT_SDK_EXPECTED_CACHE",
                os_cache.join(format!(
                    "omni-knowledge-derived-{}",
                    rustix::process::geteuid().as_raw()
                )),
            )
            .env_remove("OMNI_INIT_SDK_REFUSE")
            .env_remove("OMNI_CACHE_DIR")
            .env("XDG_CACHE_HOME", &cache)
            .env("HOME", &fake_home)
            .env("TMPDIR", temporary.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh SDK startup: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Refusing an overlapping aliased cache must precede creating anything in the authored home.
#[test]
fn init_refuses_aliased_internal_cache_before_creation() {
    if sdk_startup_child() {
        return;
    }
    let fixture = tempfile::tempdir().unwrap();
    for explicit in [false, true] {
        let root = fixture.path().join(if explicit {
            "explicit-library"
        } else {
            "os-library"
        });
        fs::create_dir(&root).unwrap();
        let alias = fixture.path().join(if explicit {
            "explicit-alias"
        } else {
            "os-alias"
        });
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
        command
            .arg("init")
            .arg(&root)
            .args(["--name", "Synthetic cache alias"])
            .env_remove("OMNI_CACHE_DIR")
            .env("HOME", fixture.path())
            .env("XDG_CACHE_HOME", &alias);
        if explicit {
            command.env("OMNI_CACHE_DIR", alias.join("derived"));
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("outside the authored library"));
        assert!(!root.join("derived").exists());
        assert!(!fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("omni-knowledge-derived-")
        }));
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "init_refuses_aliased_internal_cache_before_creation",
                "--nocapture",
            ])
            .env("OMNI_INIT_SDK_ROOT", &root)
            .env("OMNI_INIT_SDK_REFUSE", "1")
            .env(
                "OMNI_INIT_SDK_EXPECTED_CACHE",
                if explicit {
                    alias.join("derived")
                } else {
                    alias.join(format!(
                        "omni-knowledge-derived-{}",
                        rustix::process::geteuid().as_raw()
                    ))
                },
            )
            .env_remove("OMNI_CACHE_DIR")
            .env("HOME", fixture.path())
            .env("XDG_CACHE_HOME", &alias);
        if explicit {
            command.env("OMNI_CACHE_DIR", alias.join("derived"));
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "SDK alias refusal: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
