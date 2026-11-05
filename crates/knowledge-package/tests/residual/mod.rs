//! Public-API regressions using bounded, valid, owned native files only.
mod compatibility;
mod files_budget;
mod foreign;
mod frozen;
mod phase2;
mod stored_controls;

use super::*;
use sha2::{Digest, Sha256};

/// Remove only this test's own fixture after all native resources and holdings drop.
struct Fixture(PathBuf);
impl Fixture {
    /// Allocate one isolated fixture directory.
    fn new(name: &str) -> Self {
        Self(dir(name))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove owned fixture");
    }
}

/// Hash real file bytes with a fixed-size window for the independent content oracle.
fn file_hash(path: &Path) -> Vec<u8> {
    let mut file = File::open(path).unwrap();
    let mut sha = Sha256::new();
    let mut bytes = [0u8; 8192];
    loop {
        let n = file.read(&mut bytes).unwrap();
        if n == 0 {
            break;
        }
        sha.update(&bytes[..n]);
    }
    sha.finalize().to_vec()
}

/// Write a bounded regular original file; this is setup, not a synthetic SUT reader.
fn original(root: &Path) -> PathBuf {
    let path = root.join("original.bin");
    let mut file = File::create(&path).unwrap();
    let block: Vec<u8> = (0..8192).map(|n| ((n * 31 + n / 17) % 251) as u8).collect();
    for _ in 0..512 {
        file.write_all(&block).unwrap();
    }
    file.sync_all().unwrap();
    path
}

/// Capture an ordinary authored page to obtain a genuine manifest and ZIP fixture.
fn page_archive(root: &Path, limits: &Bounds) -> PathBuf {
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let path = root.join("page.md");
    fs::write(&path, b"# Owned page\nnormal local content\n").unwrap();
    let mut file = File::open(path).unwrap();
    let destination = root.join("page.zip");
    let mut request = base_capture(&private, &destination);
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut file,
    }];
    let cancel = AtomicBool::new(false);
    {
        let result = run(limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).unwrap()
        });
        assert!(result.linked);
    }
    destination
}

/// A real 4 MiB original traverses capture/inspect/stage/apply under a 3 MiB call ceiling.
#[test]
fn real_original_streams_through_public_ports_below_payload_memory() {
    let fixture = Fixture::new("stream-native");
    let root = &fixture.0;
    let source = original(root);
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let destination = root.join("out.zip");
    let mut limits = bounds(12_000_000);
    limits.budget.max_memory_bytes = 3_000_000;
    limits.quotas.max_entry_expanded = 8_000_000;
    limits.quotas.max_total_expanded = 24_000_000;
    assert!(fs::metadata(&source).unwrap().len() > limits.budget.max_memory_bytes);
    let mut input = File::open(&source).unwrap();
    let mut inputs = [Input {
        path: rel("media/original.bin"),
        role: Role::Original,
        mount_id: Some("mount".into()),
        source_id: Some("source".into()),
        expected: None,
        reader: &mut input,
    }];
    let mut request = base_capture(&private, &destination);
    let cancel = AtomicBool::new(false);
    {
        let result = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).unwrap()
        });
        assert!(result.linked);
    }
    {
        let mut archive = File::open(&destination).unwrap();
        let result = run(&limits, &cancel, |job| {
            inspect(&mut archive, job, &[]).unwrap()
        });
        assert_eq!(
            result.manifest.entries[0].size_bytes,
            fs::metadata(&source).unwrap().len()
        );
    }
    let mut archive = File::open(&destination).unwrap();
    let mut staged = run(&limits, &cancel, |job| {
        stage(&mut archive, job, &private, "restore", &[]).unwrap()
    });
    let mut home = Home {
        root: root.join("home"),
        calls: Vec::new(),
    };
    fs::create_dir(&home.root).unwrap();
    let mut history = HistorySpy { calls: 0 };
    let applied = staged
        .with_work(&limits, &cancel, |s, job| {
            apply(s, &mut home, &mut history, job)
        })
        .unwrap();
    assert_eq!(applied.state, ApplyState::Completed);
    assert_eq!(
        file_hash(&source),
        file_hash(&home.root.join("media/original.bin"))
    );
    assert_eq!(
        file_hash(&source),
        file_hash(&private.join("restore/files/media/original.bin"))
    );
}

/// Optional small JSON elements need graph funding; the same valid document succeeds with room.
#[test]
fn valid_optional_graph_is_preadmitted_and_preserved() {
    let fixture = Fixture::new("metadata-native");
    let limits = bounds(1_000_000);
    let archive_path = page_archive(&fixture.0, &limits);
    let mut zip = zip::ZipArchive::new(File::open(&archive_path).unwrap()).unwrap();
    let mut canonical = Vec::new();
    zip.by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut canonical)
        .unwrap();
    let mut page = Vec::new();
    zip.by_name("pages/a.md")
        .unwrap()
        .read_to_end(&mut page)
        .unwrap();
    drop(zip);
    let mut narrow = limits;
    narrow.budget.max_memory_bytes = 512_000;
    let cancel = AtomicBool::new(false);
    {
        let mut input = File::open(&archive_path).unwrap();
        let result = run(&narrow, &cancel, |job| inspect(&mut input, job, &[]));
        assert!(result.is_ok());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    value["owned_optional"] = serde_json::json!({ "samples": vec![serde_json::Value::Null; 256] });
    let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
    let extended = fixture.0.join("extended.zip");
    fs::write(&extended, zip_with_manifest(&canonical, &page)).unwrap();
    {
        let mut input = File::open(&extended).unwrap();
        let result = run(&narrow, &cancel, |job| inspect(&mut input, job, &[]));
        assert_eq!(result.as_ref().unwrap_err(), &Error::TooLarge);
    }
    {
        let mut input = File::open(&extended).unwrap();
        let result = run(&limits, &cancel, |job| {
            inspect(&mut input, job, &[]).unwrap()
        });
        assert_eq!(
            result.manifest.extra["owned_optional"]["samples"]
                .as_array()
                .unwrap()
                .len(),
            256
        );
    }
}

/// Native archive growth, including framing, stays within the quota and preserves earlier files.
#[test]
fn streamed_extent_refusal_preserves_partial_and_previous_output() {
    let fixture = Fixture::new("extent-native");
    let source = original(&fixture.0);
    let private = fixture.0.join("private");
    fs::create_dir(&private).unwrap();
    let destination = fixture.0.join("out.zip");
    fs::write(&destination, b"previous completed output").unwrap();
    fs::write(private.join("unknown-generation"), b"unknown evidence").unwrap();
    let mut file = File::open(&source).unwrap();
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("mount".into()),
        source_id: Some("source".into()),
        expected: None,
        reader: &mut file,
    }];
    let mut limits = bounds(128_000);
    limits.quotas.max_entry_expanded = 8_000_000;
    let mut request = base_capture(&private, &destination);
    let cancel = AtomicBool::new(false);
    {
        let result = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job)
        });
        assert_eq!(result.as_ref().unwrap_err(), &Error::Quota);
    }
    let partial = private.join(format!("{}.partial", request.operation_id));
    assert!(fs::metadata(partial).unwrap().len() <= limits.quotas.max_archive_bytes);
    assert_eq!(
        fs::read(&destination).unwrap(),
        b"previous completed output"
    );
    assert_eq!(
        fs::read(private.join("unknown-generation")).unwrap(),
        b"unknown evidence"
    );
    assert_eq!(fs::metadata(source).unwrap().len(), 4 * 1024 * 1024);
}

/// Recovery refuses an appended PLAN and a read-budget shortage without altering evidence.
#[test]
fn recovery_uses_fresh_manifest_and_bounded_plan_comparison() {
    let fixture = Fixture::new("recovery-native");
    let root = &fixture.0;
    let limits = bounds(1_000_000);
    let destination = page_archive(root, &limits);
    let private = root.join("private");
    let cancel = AtomicBool::new(false);
    let mut archive = File::open(destination).unwrap();
    let mut staged = run(&limits, &cancel, |job| {
        stage(&mut archive, job, &private, "restore", &[]).unwrap()
    });
    let mut home = Home {
        root: root.join("home"),
        calls: Vec::new(),
    };
    fs::create_dir(&home.root).unwrap();
    let mut history = HistorySpy { calls: 0 };
    let mut narrow = limits;
    narrow.budget.max_bytes = 64;
    assert_eq!(
        staged.with_work(&narrow, &cancel, |s, job| apply(
            s,
            &mut home,
            &mut history,
            job
        )),
        Err(Error::Quota)
    );
    let generation = private.join("restore");
    fs::write(generation.join("unknown"), b"keep evidence").unwrap();
    let plan = generation.join("PLAN");
    OpenOptions::new()
        .append(true)
        .open(&plan)
        .unwrap()
        .write_all(b"extra\n")
        .unwrap();
    let before = fs::read(&plan).unwrap();
    assert_eq!(
        staged.with_work(&limits, &cancel, |s, job| apply(
            s,
            &mut home,
            &mut history,
            job
        )),
        Err(Error::Conflict)
    );
    assert_eq!(fs::read(plan).unwrap(), before);
    assert_eq!(
        fs::read(generation.join("unknown")).unwrap(),
        b"keep evidence"
    );
    assert!(!home.root.join("pages/a.md").exists());
}

/// A symlink component is an ordinary refusal, even when it names another owned directory.
#[test]
fn relative_ancestor_refusal_keeps_all_owned_generations() {
    let fixture = Fixture::new("ancestor-native");
    let root = &fixture.0;
    let limits = bounds(1_000_000);
    let destination = page_archive(root, &limits);
    let private = root.join("private");
    let cancel = AtomicBool::new(false);
    let mut archive = File::open(destination).unwrap();
    let mut staged = run(&limits, &cancel, |job| {
        stage(&mut archive, job, &private, "restore", &[]).unwrap()
    });
    let generation = private.join("restore/files");
    fs::rename(generation.join("pages"), generation.join("previous-pages")).unwrap();
    std::os::unix::fs::symlink("previous-pages", generation.join("pages")).unwrap();
    let mut home = Home {
        root: root.join("home"),
        calls: Vec::new(),
    };
    fs::create_dir(&home.root).unwrap();
    let mut history = HistorySpy { calls: 0 };
    assert_eq!(
        staged.with_work(&limits, &cancel, |s, job| apply(
            s,
            &mut home,
            &mut history,
            job
        )),
        Err(Error::Hostile)
    );
    assert_eq!(
        fs::read(generation.join("previous-pages/a.md")).unwrap(),
        b"# Owned page\nnormal local content\n"
    );
    assert!(!home.root.join("pages/a.md").exists());
}

/// Cancellation and expired deadlines do not create or replace an archive.
#[test]
fn capture_cooperative_refusals_preserve_previous_files() {
    let fixture = Fixture::new("cooperative-native");
    let root = &fixture.0;
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let destination = root.join("out.zip");
    fs::write(&destination, b"previous").unwrap();
    let source = root.join("page.md");
    fs::write(&source, b"normal").unwrap();
    let mut file = File::open(source).unwrap();
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut file,
    }];
    let limits = bounds(1_000_000);
    let mut request = base_capture(&private, &destination);
    let cancel = AtomicBool::new(true);
    {
        let result = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job)
        });
        assert_eq!(result.as_ref().unwrap_err(), &Error::Cancelled);
    }
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    {
        let result = run(&limits, &cancel, |job| {
            job.deadline = std::time::Instant::now();
            capture(&mut request, &mut inputs, job)
        });
        assert_eq!(result.as_ref().unwrap_err(), &Error::Deadline);
    }
    assert_eq!(fs::read(destination).unwrap(), b"previous");
    assert_eq!(fs::read_dir(private).unwrap().count(), 0);
}
