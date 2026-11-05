//! Native held-FD public contracts: real formats, path replacement, version refusal and retained credit.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_processing::admission::{self, AdmitError, Admitted};
use knowledge_processing::funding::Funding;
use knowledge_processing::held::{EpubMember, HeldSource, PartialOutputs, SourceChanged};
use knowledge_processing::{epub, media};
use sha2::{Digest, Sha256};

static GATE: Mutex<()> = Mutex::new(());
const RSS: u64 = 128 * 1024 * 1024;
const STORAGE: u64 = 2 * 1024 * 1024;

/// Observe actual fixture bytes; these data values never authorize native access.
fn digest(bytes: &[u8]) -> ContentDigest {
    ContentDigest::new(format!("{:x}", Sha256::digest(bytes))).unwrap()
}

/// Create an actual new private output, as required of the native owner.
fn output(path: &Path) -> File {
    File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap()
}

/// Destroy the consumer and native leases before releasing its original credit.
fn retire<T>(admitted: Admitted<T>) {
    drop(admitted.value);
    for mut holding in admitted.retained {
        holding.finish().unwrap();
    }
}

/// Destroy stopped partial records before releasing their actual record/storage promises.
fn retire_partial(error: anyhow::Error) {
    let partial = error.downcast::<PartialOutputs>().unwrap();
    drop(partial.artifacts);
    drop(partial.sources);
    drop(partial.directory);
    for mut holding in partial.retained {
        holding.finish().unwrap();
    }
}

/// Selected EPUB members and the actual output survive root/parent replacement; logical names resolve only selected members.
#[test]
fn epub_uses_selected_inodes_and_logical_members_after_path_replacement() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let source_root = root.path().join("sources");
    fs::create_dir(&source_root).unwrap();
    let chapter = b"# Selected chapter\n[Next](second.html#there)\n![Selected](images/pixel.png)\n![Not selected](missing.png)";
    let second = b"<h1 id='there'>Selected second</h1><script>POISON()</script>";
    let pixel = include_bytes!("../../../assets/brand/omni-mark.png");
    fs::write(source_root.join("one.md"), chapter).unwrap();
    fs::write(source_root.join("two.html"), second).unwrap();
    fs::write(source_root.join("pixel.png"), pixel).unwrap();
    let files: Vec<_> = ["one.md", "two.html", "pixel.png"]
        .map(|name| File::open(source_root.join(name)).unwrap())
        .into();
    let versions = [digest(chapter), digest(second), digest(pixel)];
    let names = ["text/first.md", "text/second.html", "text/images/pixel.png"]
        .map(|s| RelativePath::new(s).unwrap());
    let members: Vec<_> = files
        .iter()
        .zip(&versions)
        .zip(&names)
        .map(|((file, version), name)| EpubMember {
            source: HeldSource {
                file,
                expected_version: version,
            },
            logical_name: name,
        })
        .collect();
    let destination = root.path().join("output");
    fs::create_dir(&destination).unwrap();
    let file = output(&destination.join("book.epub"));
    let inode = file.metadata().unwrap().ino();
    fs::rename(&source_root, root.path().join("selected-sources")).unwrap();
    fs::create_dir(&source_root).unwrap();
    fs::write(source_root.join("one.md"), "POISON replacement source").unwrap();
    fs::rename(&destination, root.path().join("selected-output")).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("book.epub"), "POISON destination").unwrap();
    let before = admission::current().unwrap().status().unwrap().reserved;
    let result = epub::export_epub_held_funded(
        &members,
        &file,
        "Actual selected publication",
        true,
        Some(RSS),
        STORAGE,
        &mut Funding::BeforeVerification,
    )
    .unwrap();
    assert_eq!(result.value.chapters, 2);
    assert_eq!(result.value.sources.len(), 3);
    assert_eq!(result.value.file.metadata().unwrap().ino(), inode);
    let mut zip = zip::ZipArchive::new(result.value.file.try_clone().unwrap()).unwrap();
    let mut first = String::new();
    zip.by_name("OEBPS/chapter00001.xhtml")
        .unwrap()
        .read_to_string(&mut first)
        .unwrap();
    assert!(first.contains("Selected chapter"));
    assert!(first.contains("chapter00002.xhtml#there"));
    assert!(first.contains("media/asset00001.png"));
    assert!(!first.contains("missing.png"));
    roxmltree::Document::parse(&first).unwrap();
    let mut asset = Vec::new();
    zip.by_name("OEBPS/media/asset00001.png")
        .unwrap()
        .read_to_end(&mut asset)
        .unwrap();
    assert_eq!(asset, pixel);
    assert_eq!(
        fs::read(destination.join("book.epub")).unwrap(),
        b"POISON destination"
    );
    assert!(
        admission::current()
            .unwrap()
            .status()
            .unwrap()
            .reserved
            .memory_bytes
            > before.memory_bytes
    );
    drop(zip);
    retire(result);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// Read publication pages by OPF spine itemrefs. ZIP member order is not the oracle.
fn spine_bodies(zip: &mut zip::ZipArchive<File>) -> Vec<String> {
    let mut opf = String::new();
    zip.by_name("OEBPS/content.opf")
        .unwrap()
        .read_to_string(&mut opf)
        .unwrap();
    let document = roxmltree::Document::parse(&opf).unwrap();
    let mut href_by_id = std::collections::HashMap::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.tag_name().name() == "item"
            && let (Some(id), Some(href)) = (node.attribute("id"), node.attribute("href"))
        {
            href_by_id.insert(id.to_string(), href.to_string());
        }
    }
    document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "itemref")
        .map(|node| {
            let href = href_by_id
                .get(node.attribute("idref").unwrap())
                .unwrap()
                .clone();
            let mut body = String::new();
            zip.by_name(&format!("OEBPS/{href}"))
                .unwrap()
                .read_to_string(&mut body)
                .unwrap();
            roxmltree::Document::parse(&body).unwrap();
            body
        })
        .collect()
}

/// Approved `[z.md, a.md]` stays that spine and body order. Name lookup still resolves both links.
#[test]
fn approved_page_order_survives_sorted_name_lookup() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let zebra = b"# Zebra\n\nZEBRA_PAGE_BODY\n\nSee [alpha](a.md).\n";
    let alpha = b"# Alpha\n\nALPHA_PAGE_BODY\n\nSee [zebra](z.md).\n";
    fs::write(root.path().join("later.md"), zebra).unwrap();
    fs::write(root.path().join("earlier.md"), alpha).unwrap();
    let files = [
        File::open(root.path().join("later.md")).unwrap(),
        File::open(root.path().join("earlier.md")).unwrap(),
    ];
    let versions = [digest(zebra), digest(alpha)];
    let names = ["z.md", "a.md"].map(|name| RelativePath::new(name).unwrap());
    let members: Vec<_> = files
        .iter()
        .zip(&versions)
        .zip(&names)
        .map(|((file, version), name)| EpubMember {
            source: HeldSource {
                file,
                expected_version: version,
            },
            logical_name: name,
        })
        .collect();
    let destination = output(&root.path().join("book.epub"));
    let before = admission::current().unwrap().status().unwrap().reserved;
    let result = epub::export_epub_held_funded(
        &members,
        &destination,
        "Approved order",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::BeforeVerification,
    )
    .unwrap();
    assert_eq!(result.value.chapters, 2);
    let mut zip = zip::ZipArchive::new(result.value.file.try_clone().unwrap()).unwrap();
    let bodies = spine_bodies(&mut zip);
    assert_eq!(bodies.len(), 2);
    assert!(bodies[0].contains("ZEBRA_PAGE_BODY"));
    assert!(bodies[0].contains("chapter00002.xhtml"));
    assert!(!bodies[0].contains("ALPHA_PAGE_BODY"));
    assert!(bodies[1].contains("ALPHA_PAGE_BODY"));
    assert!(bodies[1].contains("chapter00001.xhtml"));
    assert!(!bodies[1].contains("ZEBRA_PAGE_BODY"));
    drop(zip);
    retire(result);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// Generate real tail-indexed MP4 so a pipe-only or empty-source fallback cannot pass.
fn video(path: &Path) {
    let result = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=96x64:r=25",
            "-t",
            "1.24",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(path).unwrap();
    assert!(
        bytes.windows(4).position(|w| w == b"moov").unwrap()
            > bytes.windows(4).position(|w| w == b"mdat").unwrap()
    );
}

/// Real FFmpeg uses seekable held input/output FDs after both source and private output parent names change.
#[test]
fn real_ffmpeg_segments_use_held_source_and_relative_exclusive_outputs() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let source_path = root.path().join("source.mp4");
    video(&source_path);
    let version = digest(&fs::read(&source_path).unwrap());
    let mut source = File::open(&source_path).unwrap();
    source.seek(SeekFrom::Start(13)).unwrap();
    let target = root.path().join("private");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let directory = File::open(&target).unwrap();
    fs::rename(&source_path, root.path().join("selected.mp4")).unwrap();
    fs::write(&source_path, b"not a movie").unwrap();
    fs::rename(&target, root.path().join("selected-private")).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(target.join("segment-000000.mp4"), b"never overwrite this").unwrap();
    let account = admission::current().unwrap();
    let status = account.status().unwrap();
    let before = status.reserved;
    let recipe = digest(b"held-video:seconds=.75;h264;640;25fps;no-audio;extent=2097152");
    let processor = digest(b"knowledge-processing:0.1.0:ffmpeg-fd");
    let key = Key {
        context: "owned-held-video-test",
        library: "video",
        mount: None,
        source: "source.mp4",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    account
        .submit(Submission {
            operation_id: "held-video",
            job_id: "held-video",
            key,
            budget: Budget {
                max_files: 8,
                max_bytes: STORAGE,
                max_memory_bytes: status.policy.memory_bytes,
                max_millis: 20_000,
            },
            reservation: Reservation {
                memory_bytes: status.memory_available,
                temporary_bytes: STORAGE,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    let result = media::segment_video_held_funded(
        HeldSource {
            file: &source,
            expected_version: &version,
        },
        &directory,
        0.75,
        media::ClipPlan {
            child_rss_bytes: Some(RSS),
            storage_bytes: STORAGE,
            max_segments: 4,
        },
        &mut Funding::Claim {
            claim: &mut claim,
            expected: key,
        },
    )
    .unwrap();
    assert_eq!(result.value.segments.len(), 2);
    assert_eq!(source.stream_position().unwrap(), 13);
    assert_eq!(
        result.value.directory.metadata().unwrap().ino(),
        directory.metadata().unwrap().ino()
    );
    for (index, segment) in result.value.segments.iter().enumerate() {
        assert_eq!(segment.start_seconds, index as f64 * 0.75);
        assert!(segment.end_seconds > segment.start_seconds);
        assert!(segment.artifact.file.metadata().unwrap().len() > 0);
        assert_eq!(
            segment.artifact.file.metadata().unwrap().ino(),
            fs::metadata(
                root.path()
                    .join("selected-private")
                    .join(&segment.artifact.name)
            )
            .unwrap()
            .ino()
        );
        let mut clip = segment.artifact.file.try_clone().unwrap();
        clip.seek(SeekFrom::Start(0)).unwrap();
        let inspected = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "fd",
                "-fd",
                "0",
                "-i",
                "fd:",
                "-show_entries",
                "stream=codec_name,width,r_frame_rate",
                "-of",
                "json",
            ])
            .stdin(Stdio::from(clip))
            .output()
            .unwrap();
        assert!(
            inspected.status.success(),
            "{}",
            String::from_utf8_lossy(&inspected.stderr)
        );
        let facts: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
        assert_eq!(facts["streams"][0]["codec_name"], "h264");
        assert_eq!(facts["streams"][0]["width"], 640);
        assert_eq!(facts["streams"][0]["r_frame_rate"], "25/1");
    }
    assert_eq!(
        fs::read(target.join("segment-000000.mp4")).unwrap(),
        b"never overwrite this"
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert!(account.status().unwrap().reserved.memory_bytes > before.memory_bytes);
    retire(result);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// A wrong/stale expected version leaves a typed stopped output owner, not an empty successful publication.
#[test]
fn changed_source_refuses_with_real_partial_output_leases() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.md");
    fs::write(&path, "expected source").unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let file = File::open(&path).unwrap();
    fs::write(&path, "changed source").unwrap();
    let destination = output(&root.path().join("book.epub"));
    let name = RelativePath::new("source.md").unwrap();
    let before = admission::current().unwrap().status().unwrap().reserved;
    let error = epub::export_epub_held_funded(
        &[EpubMember {
            source: HeldSource {
                file: &file,
                expected_version: &version,
            },
            logical_name: &name,
        }],
        &destination,
        "Version refusal",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::BeforeVerification,
    )
    .err()
    .unwrap();
    assert!(
        error
            .downcast_ref::<PartialOutputs>()
            .unwrap()
            .cause
            .downcast_ref::<SourceChanged>()
            .is_some()
    );
    assert_eq!(destination.metadata().unwrap().len(), 0);
    assert_eq!(
        error.downcast_ref::<PartialOutputs>().unwrap().sources[0]
            .metadata()
            .unwrap()
            .ino(),
        file.metadata().unwrap().ino()
    );
    retire_partial(error);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// The kernel extent limit applies before EPUB writes, while known failed outputs and credit remain inspectable.
#[test]
fn held_epub_extent_is_prepaid_and_known_partial_is_typed() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.md");
    fs::write(&path, "# Real bounded EPUB\n").unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let source = File::open(path).unwrap();
    let destination = output(&root.path().join("book.epub"));
    let name = RelativePath::new("source.md").unwrap();
    let before = admission::current().unwrap().status().unwrap().reserved;
    let error = epub::export_epub_held_funded(
        &[EpubMember {
            source: HeldSource {
                file: &source,
                expected_version: &version,
            },
            logical_name: &name,
        }],
        &destination,
        "One byte extent",
        false,
        Some(RSS),
        1,
        &mut Funding::BeforeVerification,
    )
    .err()
    .unwrap();
    assert!(destination.metadata().unwrap().len() <= 1);
    assert_eq!(
        error
            .downcast_ref::<PartialOutputs>()
            .unwrap()
            .artifacts
            .len(),
        1
    );
    retire_partial(error);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// An all-credit actual claim funds the held ABI without a second reservation; record holdings outlive execution.
#[test]
fn actual_claim_funds_held_epub_and_refusals_are_before_output() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.md");
    fs::write(&path, "# Funded actual publication\n").unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let source = File::open(path).unwrap();
    let destination = output(&root.path().join("book.epub"));
    let name = RelativePath::new("source.md").unwrap();
    let members = [EpubMember {
        source: HeldSource {
            file: &source,
            expected_version: &version,
        },
        logical_name: &name,
    }];
    let recipe = digest(b"held-epub:title=Funded;include-media=false;extent=2097152");
    let processor = digest(b"knowledge-processing:0.1.0:held-epub:mature-encoder");
    let key = Key {
        context: "owned-held-test",
        library: "publication",
        mount: None,
        source: "source.md",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    let account = admission::current().unwrap();
    let before = account.status().unwrap();
    account
        .submit(Submission {
            operation_id: "held-epub",
            job_id: "held-epub",
            key,
            budget: Budget {
                max_files: 4,
                max_bytes: STORAGE,
                max_memory_bytes: before.policy.memory_bytes,
                max_millis: 20_000,
            },
            reservation: Reservation {
                memory_bytes: before.memory_available,
                temporary_bytes: STORAGE,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(account.status().unwrap().memory_available, 0);
    let error = epub::export_epub_held_funded(
        &members,
        &destination,
        "Funded",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::BeforeVerification,
    )
    .err()
    .unwrap();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::WaitingMemory)
    );
    let error = epub::export_epub_held_funded(
        &members,
        &destination,
        "Funded",
        false,
        Some(before.policy.memory_bytes),
        STORAGE,
        &mut Funding::Claim {
            claim: &mut claim,
            expected: key,
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::TooLargeMemory)
    );
    assert_eq!(destination.metadata().unwrap().len(), 0);
    let result = epub::export_epub_held_funded(
        &members,
        &destination,
        "Funded",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::Claim {
            claim: &mut claim,
            expected: key,
        },
    )
    .unwrap();
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert!(account.status().unwrap().reserved.memory_bytes > before.reserved.memory_bytes);
    assert!(result.value.file.metadata().unwrap().len() > 0);
    retire(result);
    assert_eq!(account.status().unwrap().reserved, before.reserved);
}

/// Insufficient actual claim credit and a live owner stop are distinct pre-effect refusals with unchanged charge.
#[test]
fn held_calls_observe_exact_claim_credit_and_live_stop() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("stopped.md");
    fs::write(&path, "# Stopped actual source\n").unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let source = File::open(path).unwrap();
    let destination = output(&root.path().join("book.epub"));
    let name = RelativePath::new("stopped.md").unwrap();
    let members = [EpubMember {
        source: HeldSource {
            file: &source,
            expected_version: &version,
        },
        logical_name: &name,
    }];
    let recipe = digest(b"held-epub:stopped:extent=2097152");
    let processor = digest(b"knowledge-processing:0.1.0:held-epub:mature-encoder");
    let key = Key {
        context: "owned-held-stop-test",
        library: "publication",
        mount: None,
        source: "stopped.md",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    let account = admission::current().unwrap();
    let before = account.status().unwrap();
    account
        .submit(Submission {
            operation_id: "held-stop",
            job_id: "held-stop",
            key,
            budget: Budget {
                max_files: 4,
                max_bytes: STORAGE,
                max_memory_bytes: before.policy.memory_bytes,
                max_millis: 20_000,
            },
            reservation: Reservation {
                memory_bytes: 4096,
                temporary_bytes: STORAGE,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    let charged = account.status().unwrap().reserved;
    let error = epub::export_epub_held_funded(
        &members,
        &destination,
        "Stopped",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::Claim {
            claim: &mut claim,
            expected: key,
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        error.downcast_ref::<AdmitError>(),
        Some(&AdmitError::InsufficientCredit)
    );
    assert_eq!(account.status().unwrap().reserved, charged);
    account.queue().cancel(claim.job(), key).unwrap();
    let error = epub::export_epub_held_funded(
        &members,
        &destination,
        "Stopped",
        false,
        Some(RSS),
        STORAGE,
        &mut Funding::Claim {
            claim: &mut claim,
            expected: key,
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        error.downcast_ref::<knowledge_processing::WorkerFailure>(),
        Some(&knowledge_processing::WorkerFailure::Cancelled)
    );
    assert_eq!(destination.metadata().unwrap().len(), 0);
    assert_eq!(account.status().unwrap().reserved, charged);
    account
        .queue()
        .settle(&claim, key, Outcome::Cancelled)
        .unwrap();
    assert_eq!(account.status().unwrap().reserved, before.reserved);
}

/// A mutation after real verification/probing cannot publish a clip as the expected source version.
#[test]
fn real_video_postread_drift_keeps_stopped_artifacts_instead_of_success() {
    use std::io::Write;
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("drifting.mp4");
    video(&path);
    let source = File::open(&path).unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let mut writer = File::options().append(true).open(&path).unwrap();
    let target = root.path().join("private");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let directory = File::open(&target).unwrap();
    let marker = target.join("segment-000000.mp4");
    let before = admission::current().unwrap().status().unwrap().reserved;
    let (result, changed) = std::thread::scope(|scope| {
        let mutation = scope.spawn(|| {
            let end = Instant::now() + std::time::Duration::from_secs(5);
            while !marker.exists() && Instant::now() < end {
                std::thread::sleep(std::time::Duration::from_micros(500));
            }
            if !marker.exists() {
                return false;
            }
            // The real valid MP4 remains decodable; only its actual byte version changes after verification.
            writer.write_all(b"changed-after-verified-probe").unwrap();
            true
        });
        let result = media::segment_video_held_funded(
            HeldSource {
                file: &source,
                expected_version: &version,
            },
            &directory,
            10.0,
            media::ClipPlan {
                child_rss_bytes: Some(RSS),
                storage_bytes: STORAGE,
                max_segments: 1,
            },
            &mut Funding::BeforeVerification,
        );
        (result, mutation.join().unwrap())
    });
    let error = match result {
        Ok(result) => {
            retire(result);
            panic!("Post-verification source mutation cannot complete as the old version");
        }
        Err(error) => error,
    };
    assert!(
        changed,
        "The deciding post-probe mutation must actually execute"
    );
    let partial = error.downcast_ref::<PartialOutputs>().unwrap();
    assert!(
        partial.cause.downcast_ref::<SourceChanged>().is_some(),
        "{}",
        partial.cause
    );
    assert_eq!(partial.artifacts.len(), 1);
    assert!(partial.artifacts[0].file.metadata().unwrap().len() > 0);
    retire_partial(error);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}

/// A preexisting symlink in the actual private directory is never followed or overwritten by generated creation.
#[test]
fn relative_output_creation_refuses_symlink_without_touching_target() {
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected.mp4");
    video(&path);
    let source = File::open(&path).unwrap();
    let version = digest(&fs::read(&path).unwrap());
    let target = root.path().join("private");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    let directory = File::open(&target).unwrap();
    let sentinel = root.path().join("sentinel");
    fs::write(&sentinel, b"must not change").unwrap();
    std::os::unix::fs::symlink(&sentinel, target.join("segment-000000.mp4")).unwrap();
    let before = admission::current().unwrap().status().unwrap().reserved;
    let error = media::segment_video_held_funded(
        HeldSource {
            file: &source,
            expected_version: &version,
        },
        &directory,
        10.0,
        media::ClipPlan {
            child_rss_bytes: Some(RSS),
            storage_bytes: STORAGE,
            max_segments: 1,
        },
        &mut Funding::BeforeVerification,
    )
    .err()
    .unwrap();
    assert!(
        error
            .downcast_ref::<PartialOutputs>()
            .unwrap()
            .artifacts
            .is_empty()
    );
    assert_eq!(fs::read(&sentinel).unwrap(), b"must not change");
    assert!(
        fs::symlink_metadata(target.join("segment-000000.mp4"))
            .unwrap()
            .is_symlink()
    );
    retire_partial(error);
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}
