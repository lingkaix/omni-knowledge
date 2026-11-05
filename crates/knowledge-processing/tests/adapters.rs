//! Real account coverage for admitted files, retained holdings, and installed ffmpeg.

use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::sync::Mutex;

use knowledge_processing::admission::{self, AdmitError};
use knowledge_processing::extract;
use knowledge_processing::media::segment_video;
use knowledge_processing::worker::WorkerFailure;

static GATE: Mutex<()> = Mutex::new(());

fn started() {
    admission::establish_process_account().unwrap();
}

#[test]
fn retained_text_stays_charged_until_finish_and_matches_bytes() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.txt");
    let original = "Hello \u{1F600} incremental\n";
    fs::write(&path, original).unwrap();
    let before = admission::current()
        .unwrap()
        .status()
        .unwrap()
        .memory_available;
    let admitted = extract::extract_path(&path, 4096).unwrap();
    assert_eq!(admitted.value.text, original);
    assert_eq!(admitted.value.status, "extracted");
    let holding = admitted.retained.first().expect("live text holding");
    assert_eq!(
        holding.remaining().unwrap().memory_bytes,
        original.len() as u64
    );
    let during = admission::current()
        .unwrap()
        .status()
        .unwrap()
        .memory_available;
    assert!(during < before);
    let mut retained = admitted.retained;
    let live = retained.pop().unwrap();
    drop(retained);
    let mut live = live;
    live.finish().unwrap();
    let after = admission::current()
        .unwrap()
        .status()
        .unwrap()
        .memory_available;
    assert!(after > during);
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
}

#[test]
fn admitted_fd_survives_unlinking_the_path() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.txt");
    let original = "same inode\n";
    fs::write(&path, original).unwrap();
    let file = File::open(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let admitted =
        extract::extract_file(&file, std::path::Path::new("note.txt"), 4096, None).unwrap();
    assert_eq!(admitted.value.text, original);
    for mut holding in admitted.retained {
        holding.finish().unwrap();
    }
}

#[test]
fn oversized_source_reserves_nothing() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wide.txt");
    fs::write(&path, "abcdef").unwrap();
    let before = admission::current().unwrap().status().unwrap();
    let admitted = extract::extract_path(&path, 4).unwrap();
    assert_eq!(admitted.value.status, "metadata-only");
    assert!(admitted.value.text.is_empty());
    assert!(admitted.retained.is_empty());
    let after = admission::current().unwrap().status().unwrap();
    assert_eq!(after.memory_available, before.memory_available);
    assert_eq!(after.reserved.memory_bytes, before.reserved.memory_bytes);
}

#[test]
fn child_promise_above_the_account_does_not_spawn() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("page.html");
    fs::write(&path, "<p>Hello</p>").unwrap();
    let original = fs::read(&path).unwrap();
    let file = File::open(&path).unwrap();
    let Err(error) = extract::extract_file(&file, &path, 4096, Some(u64::MAX)) else {
        panic!("child promise above the account must be refused");
    };
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::TooLargeMemory)
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn one_byte_rss_ceiling_stops_the_parser_worker() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("page.html");
    fs::write(&path, "<p>Hello</p>").unwrap();
    let original = fs::read(&path).unwrap();
    let file = File::open(&path).unwrap();
    let Err(error) = extract::extract_file(&file, &path, 4096, Some(1)) else {
        panic!("one-byte resident ceiling must stop the parser worker");
    };
    assert!(error.chain().any(|item| {
        matches!(
            item.downcast_ref::<WorkerFailure>(),
            Some(WorkerFailure::RssExceeded { limit_bytes: 1, .. } | WorkerFailure::Unmeasured)
        )
    }));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn xlsx_worker_keeps_shared_strings() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sheet.xlsx");
    let file = File::create(&path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("xl/sharedStrings.xml", options).unwrap();
    archive
        .write_all(br#"<sst><si><t>Alpha</t></si><si><t>Beta</t></si></sst>"#)
        .unwrap();
    archive
        .start_file("xl/worksheets/sheet1.xml", options)
        .unwrap();
    archive
        .write_all(
            br#"<worksheet><sheetData><row><c t="s"><v>0</v></c><c t="s"><v>1</v></c></row></sheetData></worksheet>"#,
        )
        .unwrap();
    archive.finish().unwrap();
    let original = fs::read(&path).unwrap();
    let admitted = extract::extract_path(&path, 65536).unwrap();
    assert!(
        admitted.value.text.contains("Alpha"),
        "{}",
        admitted.value.text
    );
    assert!(
        admitted.value.text.contains("Beta"),
        "{}",
        admitted.value.text
    );
    assert_ne!(admitted.value.status, "failed");
    assert_eq!(fs::read(&path).unwrap(), original);
    for mut holding in admitted.retained {
        holding.finish().unwrap();
    }
}

#[test]
fn ffmpeg_segments_use_storage_and_leave_the_source() {
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
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
    let output = directory.path().join("clips");
    fs::create_dir(&output).unwrap();
    let before = admission::current()
        .unwrap()
        .status()
        .unwrap()
        .storage_available;
    let segments = segment_video(&source, &output, 1.0).unwrap();
    assert!(segments.len() >= 2, "{}", segments.len());
    let after = admission::current()
        .unwrap()
        .status()
        .unwrap()
        .storage_available;
    assert!(after >= before.saturating_sub(8 * 1024 * 1024));
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(output.join("segment-000000.mp4").is_file());
    let again = output.join("segment-000000.mp4");
    let error = segment_video(&source, &output, 1.0).unwrap_err();
    assert!(
        format!("{error:#}").contains(&again.display().to_string())
            || format!("{error:#}").contains("already exists")
    );
}

/// Actual non-reaping native exit, group/pipe collection and credit lifetime; no BSD-data stub.
#[cfg(target_os = "macos")]
#[test]
fn real_owned_darwin_exit_keeps_child_and_credit_through_collection() {
    use knowledge_core::admission::{Decision, Origin};
    use knowledge_core::jobs::Reservation;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let _gate = GATE.lock().unwrap_or_else(|error| error.into_inner());
    started();
    let account = admission::current().unwrap();
    let mut holding = match account
        .reserve(
            Reservation {
                memory_bytes: 64 * 1024 * 1024,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(reason) => panic!("native fixture waiting: {reason:?}"),
    };
    let mut child = Command::new("/usr/bin/true")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    let mut observed_exit = false;
    while Instant::now() < end {
        if rustix::process::waitid(
            rustix::process::WaitId::Pid(pid),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOWAIT
                | rustix::process::WaitIdOptions::NOHANG,
        )
        .is_ok_and(|status| status.is_some())
        {
            observed_exit = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // Keep the real child and paid credit foreground/reachable on an unknown collection.
    while let Err(error) = knowledge_processing::worker::settle(&mut child) {
        eprintln!(
            "native fixture retains actual child {} and credit: {error}",
            child.id()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(child.try_wait().unwrap().is_some());
    drop(child);
    holding.finish().unwrap();
    assert!(
        observed_exit,
        "matching native WNOWAIT exit was not observed"
    );
}
