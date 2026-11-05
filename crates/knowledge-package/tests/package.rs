use crc32fast::Hasher;
use flate2::Compression;
use flate2::write::DeflateEncoder;
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_package::{
    Admit, ApplyOutcome, ApplyState, Availability, Bounds, Capture, DigestState, Error, Expansion,
    ExternalRef, GitAdmit, GitTransactions, Input, Method, Preset, Quotas, Role, SafeApply,
    V1HostContext, V1State, Window, Work, apply, capture, inspect, stage, v1_import_plan,
};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Ledger {
    mem_limit: u64,
    mem: AtomicU64,
    store_limit: u64,
    promised: AtomicU64,
}

impl Ledger {
    fn wide() -> Self {
        Self {
            mem_limit: 64 * 1024 * 1024,
            mem: AtomicU64::new(0),
            store_limit: 64 * 1024 * 1024,
            promised: AtomicU64::new(0),
        }
    }
    fn used(&self) -> u64 {
        self.mem.load(Ordering::Relaxed)
    }
}

impl Expansion for Ledger {
    fn grow(&mut self, memory_bytes: u64, temporary_bytes: u64) -> Result<Admit, Error> {
        if memory_bytes > self.mem_limit || temporary_bytes > self.store_limit {
            return Err(Error::TooLarge);
        }
        let mem = self.mem.load(Ordering::Relaxed);
        let promised = self.promised.load(Ordering::Relaxed);
        if mem.saturating_add(memory_bytes) > self.mem_limit
            || promised.saturating_add(temporary_bytes) > self.store_limit
        {
            return Ok(Admit::Waiting);
        }
        self.mem.fetch_add(memory_bytes, Ordering::Relaxed);
        self.promised.fetch_add(temporary_bytes, Ordering::Relaxed);
        Ok(Admit::Ready)
    }
    fn written(&mut self, bytes: u64) -> Result<(), Error> {
        let promised = self.promised.load(Ordering::Relaxed);
        if bytes > promised {
            return Err(Error::Invalid);
        }
        self.promised.fetch_sub(bytes, Ordering::Relaxed);
        Ok(())
    }
}

struct Home {
    root: PathBuf,
    calls: Vec<String>,
}
impl SafeApply for Home {
    fn apply_text_change(
        &mut self,
        _: &str,
        path: &RelativePath,
        _: Option<&ContentDigest>,
        candidate: &Path,
    ) -> Result<ApplyOutcome, Error> {
        self.calls.push(path.as_str().to_owned());
        place(&self.root, path, candidate, true)
    }
    fn install_new(
        &mut self,
        _: &str,
        path: &RelativePath,
        candidate: &Path,
    ) -> Result<ApplyOutcome, Error> {
        self.calls.push(path.as_str().to_owned());
        place(&self.root, path, candidate, false)
    }
}
struct HistorySpy {
    calls: u32,
}
impl GitTransactions for HistorySpy {
    fn admit_inert_bundle(&mut self, _: &str, bundle: &Path) -> Result<GitAdmit, Error> {
        self.calls += 1;
        let mut file = File::open(bundle).map_err(|_| Error::Io)?;
        let mut header = [0u8; 4];
        file.read_exact(&mut header).map_err(|_| Error::Io)?;
        assert_eq!(&header, b"PACK");
        Ok(GitAdmit::Inert)
    }
}

fn place(
    root: &Path,
    path: &RelativePath,
    candidate: &Path,
    replace_absent_only: bool,
) -> Result<ApplyOutcome, Error> {
    let _ = replace_absent_only;
    let dest = root.join(path.as_str());
    if dest.exists() {
        return Ok(ApplyOutcome::Conflict);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| Error::Io)?;
    }
    let bytes = fs::read(candidate).map_err(|_| Error::Io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|_| Error::Io)?;
    output.write_all(&bytes).map_err(|_| Error::Io)?;
    Ok(ApplyOutcome::Saved)
}

fn bounds(archive: u64) -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 1_000,
            max_bytes: 80_000_000,
            max_millis: 30_000,
            max_memory_bytes: 32_000_000,
        },
        quotas: Quotas {
            max_archive_bytes: archive,
            max_entries: 100,
            max_entry_expanded: 2_000_000,
            max_total_expanded: 8_000_000,
            max_ratio: 1_000,
            max_path_bytes: 256,
            max_depth: 8,
        },
    }
}

fn work<'a>(bounds: &'a Bounds, ledger: &'a mut Ledger, cancel: &'a AtomicBool) -> Work<'a> {
    Work::new(bounds, ledger, cancel).expect("work")
}

fn rel(value: &str) -> RelativePath {
    RelativePath::new(value).expect("path")
}

fn dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("pkg-{name}-{nanos}"));
    fs::create_dir(&path).expect("dir");
    path
}

fn window() -> Window {
    Window {
        start: "2026-11-05T00:00:00Z".into(),
        end: "2026-11-05T00:00:01Z".into(),
    }
}

#[test]
fn roundtrip_preserves_bytes_and_skips_history_index_and_conflicts() {
    let root = dir("round");
    let private = root.join("private");
    let home = root.join("home");
    fs::create_dir(&private).unwrap();
    fs::create_dir(&home).unwrap();
    fs::write(home.join("pages").join("keep.md"), b"keep").ok();
    fs::create_dir(home.join("pages")).unwrap();
    fs::write(home.join("pages").join("keep.md"), b"keep").unwrap();
    let dest = root.join("out.zip");
    let text = b"a\r\nb\n";
    let original = [0u8, 255, 10, 13];
    let mut text_reader = &text[..];
    let mut original_reader = &original[..];
    let history = b"PACKh";
    let mut history_reader = &history[..];
    let index = b"INDEX";
    let mut index_reader = &index[..];
    let mut inputs = [
        Input {
            path: rel("pages/note.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut text_reader,
        },
        Input {
            path: rel("pages/keep.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut &b"new"[..],
        },
        Input {
            path: rel("media/blob.bin"),
            role: Role::Original,
            mount_id: Some("mount".into()),
            source_id: Some("source".into()),
            expected: None,
            reader: &mut original_reader,
        },
        Input {
            path: rel("history/bundle.pack"),
            role: Role::History,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut history_reader,
        },
        Input {
            path: rel("indexes/lex.bin"),
            role: Role::Index,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut index_reader,
        },
    ];
    let mut request = Capture {
        operation_id: "cap1",
        preset: Preset::Backup,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![],
        omitted: 2,
        require_complete: false,
        required_features: vec![],
        private_dir: &private,
        destination: &dest,
    };
    let limits = bounds(8_000_000);
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let mut job = work(&limits, &mut ledger, &cancel);
    let captured = capture(&mut request, &mut inputs, &mut job).expect("capture");
    assert!(captured.linked);
    let mut archive = File::open(&dest).unwrap();
    let mut ledger = Ledger::wide();
    let mut job = work(&limits, &mut ledger, &cancel);
    let preview = inspect(&mut archive, &mut job, &[rel("pages/keep.md")]).expect("inspect");
    assert_eq!(preview.conflicts, vec![rel("pages/keep.md")]);
    assert!(preview.installable);
    assert_eq!(preview.manifest.coverage.omitted, 2);
    assert!(preview.manifest.indexes.is_some());
    let mut archive = File::open(&dest).unwrap();
    let mut ledger = Ledger::wide();
    let mut job = work(&limits, &mut ledger, &cancel);
    let staged = stage(
        &mut archive,
        &mut job,
        &private,
        "restore1",
        &[rel("pages/keep.md")],
    )
    .expect("stage");
    let mut home_apply = Home {
        root: home.clone(),
        calls: vec![],
    };
    let mut git = HistorySpy { calls: 0 };
    let applied = apply(&staged, &mut home_apply, &mut git).expect("apply");
    assert_eq!(applied.state, ApplyState::Conflict);
    assert_eq!(fs::read(home.join("pages/note.md")).unwrap(), text);
    assert_eq!(fs::read(home.join("pages/keep.md")).unwrap(), b"keep");
    assert_eq!(fs::read(home.join("media/blob.bin")).unwrap(), original);
    assert!(!home.join(".git").exists());
    assert!(!home.join("indexes").exists());
    assert_eq!(git.calls, 1);
    assert!(!home_apply.calls.iter().any(|call| call == "pages/keep.md"));
    let _ = Method::Stored;
}

#[test]
fn destination_and_require_complete_do_not_clobber() {
    let root = dir("clobber");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let dest = root.join("out.zip");
    fs::write(&dest, b"keep").unwrap();
    let text = b"hello";
    let mut reader = &text[..];
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut reader,
    }];
    let mut request = Capture {
        operation_id: "cap2",
        preset: Preset::Frozen,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![ExternalRef {
            source_id: "s".into(),
            mount_id: "m".into(),
            path: rel("nas/a.bin"),
            digest_state: DigestState::Unverified,
            digest: None,
            availability: Availability::Unavailable,
            size_bytes: None,
        }],
        omitted: 0,
        require_complete: true,
        required_features: vec![],
        private_dir: &private,
        destination: &dest,
    };
    let limits = bounds(1_000_000);
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let mut job = work(&limits, &mut ledger, &cancel);
    let error = capture(&mut request, &mut inputs, &mut job).expect_err("incomplete");
    assert_eq!(error, Error::RequireComplete);
    assert_eq!(fs::read(&dest).unwrap(), b"keep");
}

#[test]
fn drift_and_forbidden_source_refuse() {
    let root = dir("drift");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let text = b"hello";
    let mut reader = &text[..];
    let wrong =
        ContentDigest::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: Some(wrong),
        reader: &mut reader,
    }];
    let dest = root.join("out.zip");
    let mut request = base_capture(&private, &dest);
    let limits = bounds(1_000_000);
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let error = capture(
        &mut request,
        &mut inputs,
        &mut work(&limits, &mut ledger, &cancel),
    )
    .expect_err("drift");
    assert_eq!(error, Error::Drift);
    assert!(!dest.exists());
}

#[test]
fn hostile_names_flags_overlap_and_checksums() {
    let limits = bounds(1_000_000);
    assert_eq!(
        inspect_raw(&classic(&[item(b"../x", b"a")]), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&classic(&[item(b"/tmp/x", b"a")]), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"A.txt", b"a"), item(b"a.txt", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"a.txt", b"a"), item(b"a.txt", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&classic(&[item(b".git/config", b"a")]), &limits),
        Err(Error::Forbidden)
    );
    let mut encrypted = item(b"a.txt", b"a");
    encrypted.flags |= 1;
    assert_eq!(
        inspect_raw(&classic(&[encrypted]), &limits),
        Err(Error::Hostile)
    );
    let mut link = item(b"a.txt", b"a");
    link.made_by = 3 << 8;
    link.external = 0o120777 << 16;
    assert_eq!(inspect_raw(&classic(&[link]), &limits), Err(Error::Hostile));
    let mut descriptor = item(b"a.txt", b"a");
    descriptor.flags |= 0x8;
    assert_eq!(
        inspect_raw(&classic(&[descriptor]), &limits),
        Err(Error::Unsupported)
    );
    let mut bad_crc = item(b"a.txt", b"hello");
    bad_crc.crc ^= 0xff;
    assert_eq!(
        inspect_raw(&classic(&[bad_crc]), &limits),
        Err(Error::Checksum)
    );
    assert_eq!(
        inspect_raw(&[0, 1, 2, 3, 4], &limits),
        Err(Error::Truncated)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"docs", b"a"), item(b"docs/a", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    let overlapped = overlap_zip();
    assert_eq!(inspect_raw(&overlapped, &limits), Err(Error::Hostile));
}

#[test]
fn declared_size_ratio_and_admission_do_not_allocate_the_lie() {
    let mut limits = bounds(1_000_000);
    limits.quotas.max_entry_expanded = 64;
    let huge = declared_zip(u32::MAX, b"hi");
    let mut ledger = Ledger::wide();
    let error = inspect_with(&huge, &limits, &mut ledger, false);
    assert_eq!(error, Err(Error::Quota));
    assert!(ledger.used() < 1_000_000);
    let bomb = deflate_bomb(40_000);
    let mut limits = bounds(1_000_000);
    limits.quotas.max_ratio = 4;
    limits.quotas.max_entry_expanded = 80_000;
    assert_eq!(inspect_raw(&bomb, &limits), Err(Error::Ratio));
    let mut tiny = Ledger {
        mem_limit: 8,
        mem: AtomicU64::new(0),
        store_limit: 1_000_000,
        promised: AtomicU64::new(0),
    };
    let limits = bounds(1_000_000);
    assert_eq!(
        inspect_with(
            &classic(&[item(b"a.txt", b"hi")]),
            &limits,
            &mut tiny,
            false
        ),
        Err(Error::TooLarge)
    );
    let mut waiting = Ledger::wide();
    waiting.mem.store(waiting.mem_limit - 4, Ordering::Relaxed);
    assert_eq!(
        inspect_with(
            &classic(&[item(b"a.txt", b"hi")]),
            &limits,
            &mut waiting,
            false
        ),
        Err(Error::Waiting)
    );
    assert_eq!(
        inspect_with(
            &classic(&[item(b"a.txt", b"hi")]),
            &limits,
            &mut Ledger::wide(),
            true
        ),
        Err(Error::Cancelled)
    );
}

#[test]
fn zip64_sparse_offset_reads_the_payload() {
    let root = dir("zip64");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let plain = root.join("plain.zip");
    let payload = b"hi";
    let mut reader = &payload[..];
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("mount".into()),
        source_id: Some("source".into()),
        expected: None,
        reader: &mut reader,
    }];
    let mut request = base_capture(&private, &plain);
    request.operation_id = "zip64src";
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    capture(
        &mut request,
        &mut inputs,
        &mut work(&limits, &mut Ledger::wide(), &cancel),
    )
    .unwrap();
    let mut plain_zip = zip::ZipArchive::new(File::open(&plain).unwrap()).unwrap();
    let mut manifest = String::new();
    plain_zip
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let path = root.join("sparse.zip");
    write_sparse_zip64(&path, manifest.as_bytes(), b"media/a.bin", payload);
    let len = fs::metadata(&path).unwrap().len();
    assert!(len > (1 << 32));
    let mut limits = bounds(len);
    limits.budget.max_bytes = len;
    let mut archive = File::open(&path).unwrap();
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let preview =
        inspect(&mut archive, &mut work(&limits, &mut ledger, &cancel), &[]).expect("zip64");
    assert_eq!(preview.manifest.entries.len(), 1);
    assert_eq!(preview.manifest.entries[0].size_bytes, 2);
    assert!(ledger.used() < 2_000_000);
}

#[test]
fn interrupted_generation_is_not_replayed() {
    let root = dir("stop");
    let private = root.join("private");
    fs::create_dir_all(private.join("op")).unwrap();
    File::create(private.join("op").join("INCOMPLETE")).unwrap();
    fs::write(private.join("op").join("partial.bin"), b"half").unwrap();
    let bytes = classic(&[item(b"a.txt", b"hi")]);
    let mut file = File::open(write_bytes(&root.join("a.zip"), &bytes)).unwrap();
    let limits = bounds(1_000_000);
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let error = stage(
        &mut file,
        &mut work(&limits, &mut ledger, &cancel),
        &private,
        "op",
        &[],
    )
    .expect_err("interrupted");
    assert_eq!(error, Error::Interrupted);
    assert_eq!(
        fs::read(private.join("op").join("partial.bin")).unwrap(),
        b"half"
    );
}

#[test]
fn foreign_v1_is_hashed_and_refused() {
    let root = dir("v1");
    fs::write(
        root.join("manifest.json"),
        b"{\"format\":\"openkit.knowledge-package\"}",
    )
    .unwrap();
    fs::write(root.join("page.md"), b"body\r\n").unwrap();
    let limits = bounds(1_000_000);
    let mut ledger = Ledger::wide();
    let cancel = AtomicBool::new(false);
    let plan = v1_import_plan(&root, None, &mut work(&limits, &mut ledger, &cancel)).expect("v1");
    assert_eq!(plan.state, V1State::CompatibilityRefused);
    assert!(!plan.host_context);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(fs::read(root.join("page.md")).unwrap(), b"body\r\n");
    let host = V1HostContext {
        owner: "o".into(),
        revision: "r".into(),
        profile: "p".into(),
    };
    let again = v1_import_plan(
        &root,
        Some(&host),
        &mut work(&limits, &mut Ledger::wide(), &cancel),
    )
    .unwrap();
    assert_eq!(again.state, V1State::CompatibilityRefused);
    assert!(again.host_context);
}

struct Item<'a> {
    name: &'a [u8],
    flags: u16,
    method: u16,
    data: &'a [u8],
    compressed: Vec<u8>,
    crc: u32,
    external: u32,
    made_by: u16,
}

fn item<'a>(name: &'a [u8], data: &'a [u8]) -> Item<'a> {
    Item {
        name,
        flags: 1 << 11,
        method: 0,
        data,
        compressed: data.to_vec(),
        crc: crc(data),
        external: 0,
        made_by: 0,
    }
}

fn crc(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

fn classic(items: &[Item<'_>]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0u32;
    for item in items {
        let local = local_header(item, 0);
        offset += local.len() as u32 + item.compressed.len() as u32;
        locals.extend(local);
        locals.extend(&item.compressed);
    }
    let cd_offset = locals.len() as u32;
    for (item, local_offset) in items.iter().zip(local_offsets(items)) {
        central.extend(central_header(item, local_offset, &[]));
        let _ = local_offset;
    }
    let mut out = locals;
    let cd_size = central.len() as u32;
    out.extend(central);
    out.extend(eocd(items.len() as u16, cd_size, cd_offset));
    let _ = offset;
    out
}

fn local_offsets(items: &[Item<'_>]) -> Vec<u32> {
    let mut at = 0u32;
    let mut offsets = Vec::new();
    for item in items {
        offsets.push(at);
        at += 30 + item.name.len() as u32 + item.compressed.len() as u32;
    }
    offsets
}

fn local_header(item: &Item<'_>, extra: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x04034b50u32.to_le_bytes());
    bytes.extend(20u16.to_le_bytes());
    bytes.extend(item.flags.to_le_bytes());
    bytes.extend(item.method.to_le_bytes());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend(item.crc.to_le_bytes());
    bytes.extend((item.compressed.len() as u32).to_le_bytes());
    bytes.extend((item.data.len() as u32).to_le_bytes());
    bytes.extend((item.name.len() as u16).to_le_bytes());
    bytes.extend(extra.to_le_bytes());
    bytes.extend(item.name);
    bytes
}

fn central_header(item: &Item<'_>, offset: u32, extra: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x02014b50u32.to_le_bytes());
    bytes.extend(item.made_by.to_le_bytes());
    bytes.extend(20u16.to_le_bytes());
    bytes.extend(item.flags.to_le_bytes());
    bytes.extend(item.method.to_le_bytes());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend(item.crc.to_le_bytes());
    bytes.extend((item.compressed.len() as u32).to_le_bytes());
    bytes.extend((item.data.len() as u32).to_le_bytes());
    bytes.extend((item.name.len() as u16).to_le_bytes());
    bytes.extend((extra.len() as u16).to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(item.external.to_le_bytes());
    bytes.extend(offset.to_le_bytes());
    bytes.extend(item.name);
    bytes.extend(extra);
    bytes
}

fn eocd(entries: u16, cd_size: u32, cd_offset: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x06054b50u32.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(entries.to_le_bytes());
    bytes.extend(entries.to_le_bytes());
    bytes.extend(cd_size.to_le_bytes());
    bytes.extend(cd_offset.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes
}

fn overlap_zip() -> Vec<u8> {
    let first = item(b"a.txt", b"0123456789abcdef");
    let second = item(b"b.txt", b"zzzz");
    let mut bytes = local_header(&first, 0);
    bytes.extend(&first.compressed);
    let second_at = 40u32;
    let second_local = local_header(&second, 0);
    let second_end = second_at as usize + second_local.len() + second.compressed.len();
    if bytes.len() < second_end {
        bytes.resize(second_end, 0);
    }
    bytes[second_at as usize..second_at as usize + second_local.len()]
        .copy_from_slice(&second_local);
    let payload_at = second_at as usize + second_local.len();
    bytes[payload_at..payload_at + second.compressed.len()].copy_from_slice(&second.compressed);
    let cd_offset = bytes.len() as u32;
    let mut central = central_header(&first, 0, &[]);
    central.extend(central_header(&second, second_at, &[]));
    let cd_size = central.len() as u32;
    bytes.extend(central);
    bytes.extend(eocd(2, cd_size, cd_offset));
    bytes
}

fn declared_zip(declared: u32, payload: &[u8]) -> Vec<u8> {
    let mut item = item(b"a.txt", payload);
    item.data = payload;
    let mut bytes = classic(&[item]);
    // Both local and central uncompressed fields sit at fixed offsets for one stored entry.
    let local_uncomp = 22;
    bytes[local_uncomp..local_uncomp + 4].copy_from_slice(&declared.to_le_bytes());
    let cd = bytes.len() - 22 - 46 - b"a.txt".len();
    let central_uncomp = cd + 24;
    bytes[central_uncomp..central_uncomp + 4].copy_from_slice(&declared.to_le_bytes());
    bytes
}

fn deflate_bomb(count: usize) -> Vec<u8> {
    let data = vec![b'a'; count];
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&data).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut item = item(b"a.txt", &data);
    item.method = 8;
    item.compressed = compressed;
    classic(std::slice::from_ref(&item))
}

fn write_sparse_zip64(path: &Path, manifest: &[u8], name: &[u8], payload: &[u8]) {
    let offset = 1u64 << 32;
    let payload_crc = crc(payload);
    let manifest_crc = crc(manifest);
    let mut manifest_local = local_header(
        &Item {
            name: b"manifest.json",
            flags: 1 << 11,
            method: 0,
            data: manifest,
            compressed: manifest.to_vec(),
            crc: manifest_crc,
            external: 0,
            made_by: 45,
        },
        0,
    );
    manifest_local.extend(manifest);
    let mut local = Vec::new();
    local.extend(0x04034b50u32.to_le_bytes());
    local.extend(45u16.to_le_bytes());
    local.extend((1u16 << 11).to_le_bytes());
    local.extend(0u16.to_le_bytes());
    local.extend([0, 0, 0, 0]);
    local.extend(payload_crc.to_le_bytes());
    local.extend(2u32.to_le_bytes());
    local.extend(2u32.to_le_bytes());
    local.extend((name.len() as u16).to_le_bytes());
    local.extend(0u16.to_le_bytes());
    local.extend(name);
    local.extend(payload);
    let cd_offset = offset + local.len() as u64;
    let extra = zip64_offset(offset);
    let manifest_central = central_header(
        &Item {
            name: b"manifest.json",
            flags: 1 << 11,
            method: 0,
            data: manifest,
            compressed: manifest.to_vec(),
            crc: manifest_crc,
            external: 0,
            made_by: 45,
        },
        0,
        &[],
    );
    let central = central_header(
        &Item {
            name,
            flags: 1 << 11,
            method: 0,
            data: payload,
            compressed: payload.to_vec(),
            crc: payload_crc,
            external: 0,
            made_by: 45,
        },
        u32::MAX,
        &extra,
    );
    // central_header wrote the real compressed length but offset MAX. Extra carries the real offset.
    let mut central_all = manifest_central;
    central_all.extend(central);
    let cd_size = central_all.len() as u64;
    let zip64_at = cd_offset + cd_size;
    let mut zip64 = Vec::new();
    zip64.extend(0x06064b50u32.to_le_bytes());
    zip64.extend(44u64.to_le_bytes());
    zip64.extend(45u16.to_le_bytes());
    zip64.extend(45u16.to_le_bytes());
    zip64.extend(0u32.to_le_bytes());
    zip64.extend(0u32.to_le_bytes());
    zip64.extend(2u64.to_le_bytes());
    zip64.extend(2u64.to_le_bytes());
    zip64.extend(cd_size.to_le_bytes());
    zip64.extend(cd_offset.to_le_bytes());
    let mut locator = Vec::new();
    locator.extend(0x07064b50u32.to_le_bytes());
    locator.extend(0u32.to_le_bytes());
    locator.extend(zip64_at.to_le_bytes());
    locator.extend(1u32.to_le_bytes());
    let mut eocd = Vec::new();
    eocd.extend(0x06054b50u32.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    eocd.extend(u16::MAX.to_le_bytes());
    eocd.extend(u16::MAX.to_le_bytes());
    eocd.extend(u32::MAX.to_le_bytes());
    eocd.extend(u32::MAX.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    let len = zip64_at + zip64.len() as u64 + locator.len() as u64 + eocd.len() as u64;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.set_len(len).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&manifest_local).unwrap();
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(&local).unwrap();
    file.write_all(&central_all).unwrap();
    file.write_all(&zip64).unwrap();
    file.write_all(&locator).unwrap();
    file.write_all(&eocd).unwrap();
    file.sync_all().unwrap();
}

fn zip64_offset(offset: u64) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend(1u16.to_le_bytes());
    extra.extend(8u16.to_le_bytes());
    extra.extend(offset.to_le_bytes());
    extra
}

fn inspect_raw(bytes: &[u8], limits: &Bounds) -> Result<(), Error> {
    let mut ledger = Ledger::wide();
    inspect_with(bytes, limits, &mut ledger, false).map(|_| ())
}

fn inspect_with(
    bytes: &[u8],
    limits: &Bounds,
    ledger: &mut Ledger,
    cancel: bool,
) -> Result<(), Error> {
    let root = dir("raw");
    let path = write_bytes(&root.join("a.zip"), bytes);
    let mut file = File::open(path).unwrap();
    let flag = AtomicBool::new(cancel);
    inspect(&mut file, &mut work(limits, ledger, &flag), &[]).map(|_| ())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> PathBuf {
    fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

fn base_capture<'a>(private_dir: &'a Path, destination: &'a Path) -> Capture<'a> {
    Capture {
        operation_id: "cap",
        preset: Preset::Frozen,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: false,
        required_features: vec![],
        private_dir,
        destination,
    }
}
