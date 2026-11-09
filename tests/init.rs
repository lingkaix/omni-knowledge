//! Built CLI initialization over real isolated cache and temporary filesystem locations.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::Command;

/// Fresh initialization must index and remain searchable when TMPDIR is on another volume.
/// Storage owns one prepaid generated volume; selecting a temporary directory is not target binding.
#[test]
fn fresh_cli_init_selects_the_actual_publication_volume() {
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
    }
}

/// Refusing an overlapping aliased cache must precede creating anything in the authored home.
#[test]
fn init_refuses_aliased_internal_cache_before_creation() {
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
    }
}
