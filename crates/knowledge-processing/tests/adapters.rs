//! Admitted adapters, legacy bypass, and installed ffmpeg/ffprobe behavior.

use std::fs;
use std::process::Command;

use knowledge_processing::admission::{AdmitError, Bound, Charge, SharedAccount};
use knowledge_processing::extract;
use knowledge_processing::media::{segment_video, segment_video_admitted};
use knowledge_processing::worker::WorkerFailure;

struct Ledger {
    memory: u64,
    storage: u64,
    charges: Vec<&'static str>,
}

impl SharedAccount for Ledger {
    fn reserve(&mut self, charge: Charge) -> Result<(), AdmitError> {
        self.charges.push(match charge {
            Charge::Input { .. } => "input",
            Charge::Work { .. } => "work",
            Charge::Output { .. } => "output",
            Charge::Live { .. } => "live",
            Charge::Storage { .. } => "storage",
        });
        let need = charge.bytes();
        if charge.is_storage() {
            if self.storage < need {
                return Err(AdmitError::TooLargeStorage);
            }
            self.storage -= need;
        } else if self.memory < need {
            return Err(AdmitError::TooLargeMemory);
        } else {
            self.memory -= need;
        }
        Ok(())
    }
}

fn ledger(memory: u64, storage: u64) -> Ledger {
    Ledger {
        memory,
        storage,
        charges: Vec::new(),
    }
}

#[test]
fn admitted_text_matches_legacy_and_refusal_leaves_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.txt");
    let original = "Hello \u{1F600} incremental\n";
    fs::write(&path, original).unwrap();
    let legacy = extract::extract(&path, 4096).unwrap();
    let mut account = ledger(u64::MAX / 4, 0);
    let admitted = extract::extract_admitted(&path, 4096, &mut account).unwrap();
    assert_eq!(legacy.text, original);
    assert_eq!(admitted.text, legacy.text);
    assert_eq!(admitted.status, "extracted");
    assert_eq!(account.charges.first().copied(), Some("input"));
    assert!(account.charges.contains(&"work"));
    assert!(account.charges.contains(&"output"));
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());

    let mut refused = ledger(0, 0);
    let error = extract::extract_admitted(&path, 4096, &mut refused).unwrap_err();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::TooLargeMemory)
    );
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
}

#[test]
fn oversized_source_stays_metadata_only_without_a_read_charge() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wide.txt");
    fs::write(&path, "abcdef").unwrap();
    let mut account = ledger(0, 0);
    let extraction = extract::extract_admitted(&path, 4, &mut account).unwrap();
    assert_eq!(extraction.status, "metadata-only");
    assert!(extraction.text.is_empty());
    assert!(account.charges.is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"abcdef");
}

#[test]
fn ffmpeg_segments_cover_ranges_and_do_not_clobber() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=blue:size=16x16:rate=25",
            "-t",
            "1.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-n",
        ])
        .arg(&source)
        .status()
        .expect("ffmpeg is required");
    assert!(status.success(), "synthetic media failed");
    let original = fs::read(&source).unwrap();
    let outputs = directory.path().join("clips");
    fs::create_dir(&outputs).unwrap();
    let mut account = ledger(u64::MAX / 4, 8 * 1024 * 1024);
    let segments = segment_video_admitted(
        &source,
        &outputs,
        1.0,
        &mut account,
        Bound {
            child_rss_bytes: None,
            storage_bytes: Some(1024 * 1024),
            deadline: None,
        },
    )
    .expect("ffprobe and ffmpeg");
    assert!(account.charges.contains(&"storage"));
    assert!(!account.charges.contains(&"live"));
    assert_eq!(segments.len(), 2);
    assert_eq!(
        (segments[0].start_seconds, segments[0].end_seconds),
        (0.0, 1.0)
    );
    assert!((segments[1].end_seconds - 1.2).abs() < 0.05);
    for segment in &segments {
        let decoded = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
            ])
            .arg(&segment.path)
            .output()
            .expect("ffprobe is required");
        assert!(decoded.status.success());
        assert!(!decoded.stdout.is_empty());
    }
    let retained = fs::read(&segments[0].path).unwrap();
    assert!(segment_video(&source, &outputs, 1.0).is_err());
    assert_eq!(fs::read(&segments[0].path).unwrap(), retained);
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn child_rss_refusal_writes_no_clip_and_keeps_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    fs::write(&source, b"not a video").unwrap();
    let original = fs::read(&source).unwrap();
    let outputs = directory.path().join("clips");
    fs::create_dir(&outputs).unwrap();
    let mut account = ledger(u64::MAX / 4, u64::MAX / 4);
    let error = segment_video_admitted(
        &source,
        &outputs,
        1.0,
        &mut account,
        Bound {
            child_rss_bytes: Some(1),
            storage_bytes: Some(1024),
            deadline: None,
        },
    )
    .unwrap_err();
    assert!(
        error.chain().any(|item| {
            matches!(
                item.downcast_ref::<WorkerFailure>(),
                Some(WorkerFailure::RssExceeded { .. } | WorkerFailure::Unmeasured)
            )
        }) || error.downcast_ref::<AdmitError>().is_some()
    );
    assert!(fs::read_dir(&outputs).unwrap().next().is_none());
    assert_eq!(fs::read(&source).unwrap(), original);
}
