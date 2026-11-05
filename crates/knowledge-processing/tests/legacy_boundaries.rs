//! Real legacy-boundary regressions: whole concurrent need and kernel-gated ffmpeg writes.

use std::fs::{self, File};
use std::process::Command;
use std::sync::Mutex;

use knowledge_processing::admission::{self, AdmitError};
use knowledge_processing::{extract, media};

static GATE: Mutex<()> = Mutex::new(());

/// An individually possible child plus its buffers must not wait forever for an impossible complete peak.
#[test]
fn complete_concurrent_parser_need_is_too_large_not_waiting() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("page.html");
    fs::write(&path, b"<p>Real input</p>").unwrap();
    let account = admission::current().unwrap();
    let before = account.status().unwrap();
    let error = extract::extract_file(
        &File::open(&path).unwrap(),
        &path,
        4096,
        Some(before.policy.memory_bytes - 1),
    )
    .err()
    .expect("impossible complete peak");
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::TooLargeMemory)
    );
    assert_eq!(account.status().unwrap().reserved, before.reserved);
    assert_eq!(fs::read(&path).unwrap(), b"<p>Real input</p>");
}

/// Observe actual generated extents, not an after-the-write counter or substituted converter.
#[test]
fn one_byte_storage_promise_cannot_generate_a_large_real_clip() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=16x16:d=1.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let original = fs::read(&source).unwrap();
    let output = root.path().join("clips");
    fs::create_dir(&output).unwrap();
    let _error = media::segment_video_file(
        &File::open(&source).unwrap(),
        &output,
        1.0,
        64 * 1024 * 1024,
        1,
    )
    .unwrap_err();
    // The real encoder ran and failed at the write ceiling, not at initial resource admission.
    let clip = output.join("segment-000000.mp4");
    assert!(clip.is_file(), "actual converter output inode must exist");
    let bytes: u64 = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    assert!(bytes <= 1, "a one-byte promise wrote {bytes} actual bytes");
    assert_eq!(fs::read(&source).unwrap(), original);
}
