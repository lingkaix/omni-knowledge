//! Real funded-file composition, retained buffer lifetimes, and valid generated-output controls.

use std::fs::{self, File};
use std::sync::Mutex;
use std::time::Instant;

use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_processing::admission::{self, AdmitError};
use knowledge_processing::funding::Funding;
use knowledge_processing::{epub, extract, media};
use sha2::{Digest, Sha256};

static GATE: Mutex<()> = Mutex::new(());

/// Hash actual owned source bytes and the actual immutable recipe strings in the native test host.
fn digest(bytes: &[u8]) -> ContentDigest {
    ContentDigest::new(format!("{:x}", Sha256::digest(bytes))).unwrap()
}

/// Fully charged real claims pay both plain copies and real opaque children by transfer, never reserve again.
#[test]
fn fully_funded_claim_consumes_credit_and_result_survives_execution_settlement() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    for (filename, source, child) in [
        ("actual.txt", "Actual funded text 😀\n", None),
        (
            "actual.html",
            "<p>Actual funded text</p>",
            Some(64 * 1024 * 1024),
        ),
    ] {
        let path = root.path().join(filename);
        fs::write(&path, source).unwrap();
        let file = File::open(&path).unwrap();
        let version = digest(&fs::read(&path).unwrap());
        let recipe =
            digest(format!("extract_file_funded:max_bytes=4096;child={child:?}").as_bytes());
        let processor = digest(b"knowledge-processing:0.1.0:UTF8/html2text-0.16.7:parser-worker");
        let key = Key {
            context: "owned-native-test-root",
            library: "funded",
            mount: None,
            source: filename,
            version: &version,
            recipe: &recipe,
            processor: &processor,
        };
        let account = admission::current().unwrap();
        let before = account.status().unwrap();
        account
            .submit(Submission {
                operation_id: filename,
                job_id: filename,
                key,
                budget: Budget {
                    max_files: 8,
                    max_bytes: 4096,
                    max_memory_bytes: before.policy.memory_bytes,
                    max_millis: 20_000,
                },
                reservation: Reservation {
                    memory_bytes: before.memory_available,
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
        assert_eq!(account.status().unwrap().memory_available, 0);
        if child.is_some() {
            let charged = account.status().unwrap().reserved;
            let error = extract::extract_file_funded(
                &file,
                &path,
                4096,
                Some(0),
                &mut Funding::Claim {
                    claim: &mut claim,
                    expected: key,
                },
            )
            .err()
            .expect("zero child credit refuses before allocation");
            assert_eq!(
                error.downcast_ref::<AdmitError>(),
                Some(&AdmitError::InsufficientCredit)
            );
            assert_eq!(account.status().unwrap().reserved, charged);
            assert_eq!(
                account
                    .job_status(claim.job(), key)
                    .unwrap()
                    .execution
                    .memory_bytes,
                before.memory_available
            );
        }
        let admitted = extract::extract_file_funded(
            &file,
            &path,
            4096,
            child,
            &mut Funding::Claim {
                claim: &mut claim,
                expected: key,
            },
        )
        .unwrap();
        if child.is_none() {
            assert_eq!(admitted.value.text, source);
            assert_eq!(
                admitted.retained[0].remaining().unwrap().memory_bytes,
                source.len() as u64
            );
        } else {
            assert!(admitted.value.text.contains("Actual funded text"));
            assert!(!admitted.value.text.contains("<p>"));
        }
        assert_eq!(admitted.value.status, "extracted");
        let retained_bytes: u64 = admitted
            .retained
            .iter()
            .map(|h| h.remaining().unwrap().memory_bytes)
            .sum();
        assert!(retained_bytes >= admitted.value.text.capacity() as u64);
        account
            .queue()
            .settle(&claim, key, Outcome::Completed)
            .unwrap();
        assert_eq!(
            account.status().unwrap().reserved.memory_bytes,
            before.reserved.memory_bytes + retained_bytes
        );
        assert!(
            admitted.value.text.contains("Actual funded text"),
            "execution settlement cannot invalidate pinned data"
        );
        drop(admitted.value);
        for mut holding in admitted.retained {
            holding.finish().unwrap();
        }
        assert_eq!(account.status().unwrap().reserved, before.reserved);
        assert_eq!(digest(&fs::read(&path).unwrap()), version);
    }
}

/// A whole-possible but currently blocked plan returns waiting with no input offset change or stranded component.
#[test]
fn contention_returns_without_partial_charge_or_descriptor_cursor_effect() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("note.txt");
    fs::write(&path, "actual bytes").unwrap();
    let file = File::open(&path).unwrap();
    let account = admission::current().unwrap();
    let mut occupied =
        admission::reserve_memory(account.status().unwrap().memory_available).unwrap();
    let before = account.status().unwrap();
    let error =
        extract::extract_file_funded(&file, &path, 4096, None, &mut Funding::BeforeVerification)
            .err()
            .unwrap();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::WaitingMemory)
    );
    assert_eq!(account.status().unwrap().reserved, before.reserved);
    use std::io::Seek;
    assert_eq!(file.try_clone().unwrap().stream_position().unwrap(), 0);
    occupied.finish().unwrap();
    let control =
        extract::extract_file_funded(&file, &path, 4096, None, &mut Funding::BeforeVerification)
            .unwrap();
    assert_eq!(control.value.text, "actual bytes");
    drop(control.value);
    for mut holding in control.retained {
        holding.finish().unwrap();
    }
}

/// Typed transcript results keep enough real credit for vector and String capacities, not just text lengths.
#[test]
fn transcript_and_preview_buffers_remain_funded_until_destroyed() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let transcript = root.path().join("actual.json");
    fs::write(
        &transcript,
        br#"{"transcription":[{"offsets":{"from":1250,"to":2750},"text":" Actual speech "}]}"#,
    )
    .unwrap();
    let before = admission::current().unwrap().status().unwrap();
    let result = media::parse_transcript_file_funded(
        &File::open(&transcript).unwrap(),
        Some(64 * 1024 * 1024),
        &mut Funding::BeforeVerification,
    )
    .unwrap();
    assert_eq!(result.value.len(), 1);
    assert_eq!(result.value[0].start_seconds, 1.25);
    assert_eq!(result.value[0].end_seconds, 2.75);
    assert_eq!(result.value[0].text, "Actual speech");
    let allocated = result.value.capacity() * std::mem::size_of::<media::TranscriptSegment>()
        + result
            .value
            .iter()
            .map(|s| s.text.capacity())
            .sum::<usize>();
    assert!(
        result
            .retained
            .iter()
            .map(|h| h.remaining().unwrap().memory_bytes)
            .sum::<u64>()
            >= allocated as u64
    );
    drop(result.value);
    for mut holding in result.retained {
        holding.finish().unwrap();
    }
    let markdown = root.path().join("note.md");
    fs::write(
        &markdown,
        "# Real preview\n<script>unsafe()</script>\nEvidence line\n",
    )
    .unwrap();
    let preview = extract::preview_file_funded(
        &File::open(&markdown).unwrap(),
        &markdown,
        4096,
        Some(3),
        Some(64 * 1024 * 1024),
        &mut Funding::BeforeVerification,
    )
    .unwrap();
    assert!(preview.value.contains("Real preview"));
    assert!(
        preview
            .value
            .contains("id=\"source-line-3\" data-preview-target=\"true\"")
    );
    assert!(!preview.value.contains("<script>"));
    assert!(
        preview.retained[0].remaining().unwrap().memory_bytes >= preview.value.capacity() as u64
    );
    drop(preview.value);
    for mut holding in preview.retained {
        holding.finish().unwrap();
    }
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before.reserved
    );
}

/// A one-byte real ZIP extent is stopped before growth; a properly funded EPUB still round-trips actual chapters.
#[test]
fn epub_generation_is_extent_gated_and_valid_control_round_trips() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let library = root.path().join("library");
    fs::create_dir(&library).unwrap();
    fs::write(
        library.join("chapter.md"),
        "# Actual chapter\n\nActual EPUB body.\n",
    )
    .unwrap();
    let failed = root.path().join("too-small.epub");
    let error = epub::export_epub_funded(
        &library,
        &failed,
        "Actual book",
        false,
        Some(64 * 1024 * 1024),
        1,
        &mut Funding::BeforeVerification,
    )
    .unwrap_err();
    assert!(!failed.exists());
    assert!(error.downcast_ref::<AdmitError>().is_none(), "{error:#}");
    let valid = root.path().join("valid.epub");
    assert_eq!(
        epub::export_epub_funded(
            &library,
            &valid,
            "Actual book",
            false,
            Some(64 * 1024 * 1024),
            1024 * 1024,
            &mut Funding::BeforeVerification
        )
        .unwrap(),
        1
    );
    let text = epub::read_epub_file_funded(
        &File::open(&valid).unwrap(),
        64 * 1024,
        Some(64 * 1024 * 1024),
        &mut Funding::BeforeVerification,
    )
    .unwrap();
    assert!(text.value.contains("Actual chapter"));
    assert!(text.value.contains("Actual EPUB body"));
    assert!(text.retained[0].remaining().unwrap().memory_bytes >= text.value.capacity() as u64);
    drop(text.value);
    for mut holding in text.retained {
        holding.finish().unwrap();
    }
    assert_eq!(
        fs::read_to_string(library.join("chapter.md")).unwrap(),
        "# Actual chapter\n\nActual EPUB body.\n"
    );
}
