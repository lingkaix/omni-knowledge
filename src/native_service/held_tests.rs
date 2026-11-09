//! Real Root callers consume the public held-FD ABI with actual native sources and scheduler Claims.

use super::*;
use knowledge_core::admission::Account;
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::{Claim, Priority, Reservation};
use std::io::{Read, Seek};
use std::os::unix::fs::FileExt;

/// Submit the complete simultaneous need to the actual process account and retain its real Claim.
fn claim(processing: &NativeProcessing, id: &str, memory: u64, storage: u64) -> Claim {
    let submitted = processing
        .submit(NativeJobPlan {
            operation_id: id,
            job_id: id,
            budget: Budget {
                max_files: 8,
                max_bytes: 16 * 1024 * 1024,
                max_memory_bytes: memory,
                max_millis: 60000,
            },
            reservation: Reservation {
                memory_bytes: memory,
                temporary_bytes: storage,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Account::current()
        .unwrap()
        .queue()
        .claim(Instant::now())
        .unwrap()
        .unwrap();
    assert_eq!(claim.job(), submitted.job);
    claim
}
/// Read a bounded generated ZIP member through the held archive, not a generated pathname reopen.
fn zip_text(zip: &mut zip::ZipArchive<&File>, name: &str) -> String {
    let mut text = String::new();
    zip.by_name(name)
        .unwrap()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .unwrap();
    text
}
/// Actual EPUB spine order, paid FD lifetimes, cross-scope member revalidation and seekable video consumption.
#[test]
fn actual_root_held_epub_and_video_keep_funded_native_owners() {
    // Preserve the process-wide credit assertions without unrelated tests
    // allocating/releasing in the same account while the two-thread gate runs.
    const CHILD: &str = "OMNI_HELD_OWNER_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_service::held_tests::actual_root_held_epub_and_video_keep_funded_native_owners",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "actual held-owner child failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(&directory.path().join("home"), "Native held caller").unwrap();
    let first = "# First\nNative first Ω 中文 chapter\n";
    let second = "# Second\nNative second Ω 中文 chapter\n";
    fs::write(kb.root.join("pages/z-first.md"), first).unwrap();
    fs::write(kb.root.join("pages/a-second.md"), second).unwrap();
    let policy = LocalPolicy {
        mode: wire::ExposureMode::ReadOnly,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let (service, _owner, _selection) =
        NativeLibraryService::ordinary_local(kb.clone(), policy).unwrap();
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let frame = serde_json::to_vec(
        &serde_json::json!({"wire_version":1,"request_id":"held-caller",
        "command":{"op":"library_status","library_id":service.library_id()}}),
    )
    .unwrap();
    let command = wire::decode_request(&frame).unwrap().command;
    let scope = || service.native_scope(&session, &command, None).unwrap();
    let verification = Budget {
        max_files: 1,
        max_bytes: 16 * 1024 * 1024,
        max_memory_bytes: 64 * 1024 * 1024,
        max_millis: 60000,
    };
    let source = |path: &str| {
        service
            .source(
                &scope(),
                &wire::RelativePath::new(path).unwrap(),
                verification,
            )
            .unwrap()
    };
    let members = vec![
        NativeEpubMember {
            source: source("pages/z-first.md"),
            name: RelativePath::new("z-first.md").unwrap(),
        },
        NativeEpubMember {
            source: source("pages/a-second.md"),
            name: RelativePath::new("a-second.md").unwrap(),
        },
    ];
    let mut recipe = Sha256::new();
    recipe.update(b"epub:Owned book:include_media=false");
    for member in &members {
        recipe.update((member.name.as_str().len() as u64).to_le_bytes());
        recipe.update(member.name.as_str().as_bytes());
        recipe.update(member.source.version().as_str().as_bytes());
    }
    let processor =
        ContentDigest::new(hex::encode(Sha256::digest(include_bytes!("held_tests.rs")))).unwrap();
    let processing = source("pages/z-first.md")
        .processing(
            ContentDigest::new(hex::encode(recipe.finalize())).unwrap(),
            processor.clone(),
        )
        .unwrap();
    let job = claim(
        &processing,
        "native-held-epub",
        64 * 1024 * 1024,
        2 * 1024 * 1024,
    );
    let book = processing
        .export_epub(
            job,
            members,
            NativeEpubPlan {
                title: "Owned book".into(),
                include_media: false,
                child_rss_bytes: None,
                storage_bytes: 1024 * 1024,
            },
        )
        .unwrap();
    assert_eq!(book.value().unwrap().chapters, 2);
    let file = &book.value().unwrap().file;
    book.source()
        .unwrap()
        .reserve_result(file.metadata().unwrap().len() * 8 + 16 * 1024)
        .unwrap();
    let mut reader = file;
    reader.rewind().unwrap();
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    let container = zip_text(&mut archive, "META-INF/container.xml");
    let container = roxmltree::Document::parse(&container).unwrap();
    let package = container
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .unwrap()
        .attribute("full-path")
        .unwrap();
    let opf = zip_text(&mut archive, package);
    let opf = roxmltree::Document::parse(&opf).unwrap();
    let chapters: Vec<_> = opf
        .descendants()
        .filter(|node| node.has_tag_name("itemref"))
        .map(|node| {
            let id = node.attribute("idref").unwrap();
            let href = opf
                .descendants()
                .find(|item| item.has_tag_name("item") && item.attribute("id") == Some(id))
                .unwrap()
                .attribute("href")
                .unwrap();
            let name = Path::new(package).parent().unwrap().join(href);
            zip_text(&mut archive, name.to_str().unwrap())
        })
        .collect();
    assert!(chapters[0].contains("Native first Ω 中文"));
    assert!(chapters[1].contains("Native second Ω 中文"));
    drop(archive);
    drop(chapters);
    let source_after = book.complete().unwrap();
    assert_eq!(
        fs::read_to_string(kb.root.join("pages/z-first.md")).unwrap(),
        first
    );
    assert_eq!(
        fs::read_to_string(kb.root.join("pages/a-second.md")).unwrap(),
        second
    );
    // Member scopes are distinct. The final anchor scope must retain the second member's real native pin.
    fs::write(
        kb.root.join("pages/a-second.md"),
        "changed after actual EPUB collection",
    )
    .unwrap();
    assert!(source_after.check().is_err());
    drop(source_after);

    let video = kb.root.join("raw/native.mp4");
    let created = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x64:rate=25",
            "-t",
            "0.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
        ])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let original = fs::read(&video).unwrap();
    let processing = source("raw/native.mp4")
        .processing(
            ContentDigest::new(hex::encode(Sha256::digest(
                b"video:contiguous:0.1s:H264:640:25fps:no-audio",
            )))
            .unwrap(),
            processor,
        )
        .unwrap();
    // Public M3's documented 256 MiB opaque promise plus simultaneous capture/metadata and staging credit.
    let job = claim(
        &processing,
        "native-held-video",
        512 * 1024 * 1024,
        16 * 1024 * 1024,
    );
    let available = Account::current()
        .unwrap()
        .status()
        .unwrap()
        .memory_available;
    let clips = processing
        .segment_video(
            job,
            NativeVideoPlan {
                seconds: 0.1,
                clips: knowledge_processing::media::ClipPlan {
                    child_rss_bytes: Some(256 * 1024 * 1024),
                    storage_bytes: 8 * 1024 * 1024,
                    max_segments: 4,
                },
            },
        )
        .unwrap_or_else(|error| panic!("funded native video failed: {:#}", error.cause()));
    let value = clips.value().unwrap();
    assert!(!value.segments.is_empty());
    assert_eq!(value.segments[0].start_seconds, 0.0);
    for segment in &value.segments {
        assert!(segment.end_seconds > segment.start_seconds);
        let mut header = [0u8; 8];
        assert_eq!(segment.artifact.file.read_at(&mut header, 0).unwrap(), 8);
        assert_eq!(&header[4..], b"ftyp");
    }
    let source_after = clips.complete().unwrap();
    assert!(
        Account::current()
            .unwrap()
            .status()
            .unwrap()
            .memory_available
            >= available + 512 * 1024 * 1024
    );
    source_after.check().unwrap();
    assert_eq!(fs::read(&video).unwrap(), original);
}
