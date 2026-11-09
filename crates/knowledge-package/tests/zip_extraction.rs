//! Dependency extraction confinement; native package restore continues to use its own owner boundary.
#![cfg(unix)]

use std::fs;
use std::io::{Cursor, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Own one private disposable root without adopting an existing temporary-directory name.
struct TempDir(PathBuf);
impl TempDir {
    /// Exclusively create a synthetic root; repeated names remain owned by their original creator.
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        loop {
            let next = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "knowledge-package-zip-extraction-{}-{next}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create private extraction fixture: {error}"),
            }
        }
    }

    /// Borrow only this test's synthetic directory.
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    /// Remove the owned fixture, including its symlinks, after extraction handles close.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Encode a real ZIP through its public writer with ordinary Unix mode metadata.
fn archive(write: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    write(&mut writer);
    writer.finish().unwrap().into_inner()
}

/// Observe permissions independently from the ZIP metadata and extraction result.
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

/// Default public extraction cannot use archive or existing links to change an outside target.
/// docs/specs/package.md requires confinement for both payload installation and destination effects.
#[test]
fn public_zip_extraction_preserves_outside_bytes_and_permissions() {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o640);
    let benign = TempDir::new();
    let bytes = archive(|writer| {
        writer.start_file("safe/payload.txt", options).unwrap();
        writer.write_all(b"ordinary extracted bytes").unwrap();
    });
    ZipArchive::new(Cursor::new(bytes))
        .unwrap()
        .extract(benign.path())
        .unwrap();
    assert_eq!(
        fs::read(benign.path().join("safe/payload.txt")).unwrap(),
        b"ordinary extracted bytes"
    );
    assert_eq!(mode(&benign.path().join("safe/payload.txt")), 0o640);

    for scenario in [
        "archive-link",
        "archive-file-link-only",
        "existing-directory-link",
        "existing-file-link",
    ] {
        let fixture = TempDir::new();
        let destination = fixture.path().join("destination");
        let outside = fixture.path().join("outside");
        fs::create_dir(&destination).unwrap();
        fs::create_dir(&outside).unwrap();
        let target = outside.join("sentinel");
        fs::write(&target, b"outside bytes stay original").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o751)).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        let original = fs::read(&target).unwrap();
        let directory_mode = mode(&outside);
        let target_mode = mode(&target);
        if scenario == "existing-directory-link" {
            symlink("../outside", destination.join("entry")).unwrap();
        } else if scenario == "existing-file-link" {
            symlink("../outside/sentinel", destination.join("entry")).unwrap();
        }
        let bytes = archive(|writer| {
            if scenario == "archive-file-link-only" {
                writer
                    .add_symlink(
                        "entry",
                        "../outside/sentinel",
                        options.unix_permissions(0o777),
                    )
                    .unwrap();
                return;
            } else if scenario == "archive-link" {
                writer
                    .add_symlink("entry", "../outside", options.unix_permissions(0o777))
                    .unwrap();
            } else if scenario == "existing-directory-link" {
                writer
                    .add_directory("entry/", options.unix_permissions(0o777))
                    .unwrap();
            }
            writer
                .start_file(
                    if scenario == "existing-file-link" {
                        "entry"
                    } else {
                        "entry/sentinel"
                    },
                    options.unix_permissions(0o666),
                )
                .unwrap();
            writer.write_all(b"archive replacement bytes").unwrap();
        });
        // Extraction can stop after a safe prefix; its actual outside footprint is the oracle.
        let _result = ZipArchive::new(Cursor::new(bytes))
            .unwrap()
            .extract(&destination);
        if scenario == "archive-link" || scenario == "archive-file-link-only" {
            // Public default extraction permits creation only for links resolving inside its root.
            match fs::symlink_metadata(destination.join("entry")) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    let resolved = destination.join("entry").canonicalize().unwrap();
                    assert!(
                        resolved.starts_with(destination.canonicalize().unwrap()),
                        "{scenario}: an archive-created link must remain confined"
                    );
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("observe extracted fixture link: {error}"),
            }
        }
        assert_eq!(
            fs::read(&target).unwrap(),
            original,
            "{scenario}: outside bytes"
        );
        assert_eq!(mode(&target), target_mode, "{scenario}: outside file mode");
        assert_eq!(
            mode(&outside),
            directory_mode,
            "{scenario}: outside directory mode"
        );
    }
}
