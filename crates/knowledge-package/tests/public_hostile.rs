use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use knowledge_core::admission::{Account, Configuration, Decision, Holding, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::RelativePath;
use knowledge_core::jobs::Reservation;
use knowledge_package::{
    ApplyOutcome, ApplyState, Bounds, Capture, Error, GitAdmit, GitTransactions, Input, Preset,
    Quotas, Role, SafeApply, VerifiedSource, Window, Work, apply, capture, inspect, stage,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn scratch_root() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory")
}

/// One process account is shared by every test. Drop does not release a holding,
/// so the gate stays held until that test has finished its reservations.
fn gate() -> MutexGuard<'static, ()> {
    static GATE: Mutex<()> = Mutex::new(());
    GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ensure_account() -> &'static Account {
    static READY: OnceLock<()> = OnceLock::new();
    READY.get_or_init(|| {
        let path = scratch_root().join(format!(
            "knowledge-package-public-account-{}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("account directory");
        let mut permissions = fs::metadata(&path).expect("account metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).expect("account mode");
        let scheduling = Scheduling {
            max_pending: 64,
            max_records: 1024,
            max_descriptor_bytes: 1 << 20,
            per_mount: 8,
        };
        let memory_bytes = Some(1 << 30);
        let open = |storage_bytes: Option<u64>| {
            let directory = File::open(&path).expect("account directory descriptor");
            Account::start(
                directory,
                Configuration {
                    memory_bytes,
                    storage_bytes,
                },
                scheduling,
            )
        };
        match open(Some(64 << 20)) {
            Ok(_) => {}
            Err(knowledge_core::admission::Error::AlreadyStarted) => {}
            Err(knowledge_core::admission::Error::TooLargeStorage) => {
                open(None).expect("account start with the native storage ceiling");
            }
            Err(error) => panic!("account start failed: {error}"),
        }
    });
    Account::current().expect("process account")
}

struct Lease {
    holding: Holding,
}

impl Lease {
    fn ready(memory_bytes: u64, temporary_bytes: u64) -> Self {
        let decision = ensure_account()
            .reserve(
                Reservation {
                    memory_bytes,
                    temporary_bytes,
                },
                Origin::Startup,
            )
            .expect("reserve");
        let holding = match decision {
            Decision::Ready(holding) => holding,
            Decision::Waiting(waiting) => panic!("pre-call reserve waited: {waiting:?}"),
        };
        Self { holding }
    }

    /// Leave the call budget free for the package to grow. Reserving the whole
    /// ceiling up front makes the first real buffer TooLarge.
    fn call() -> Self {
        Self::ready(4 << 10, 4 << 10)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Err(error) = self.holding.finish()
            && error != knowledge_core::admission::Error::Stale
            && !std::thread::panicking()
        {
            panic!("finish holding: {error}");
        }
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = scratch_root().join(format!(
            "knowledge-package-public-hostile-{}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct FsApply {
    root: PathBuf,
}

impl SafeApply for FsApply {
    fn apply_text_change(
        &mut self,
        _operation_id: &str,
        path: &RelativePath,
        _expected: Option<&knowledge_core::data::ContentDigest>,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        publish_new(&self.root, path, candidate, work)
    }

    fn install_new(
        &mut self,
        _operation_id: &str,
        path: &RelativePath,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        publish_new(&self.root, path, candidate, work)
    }
}

fn publish_new(
    root: &Path,
    path: &RelativePath,
    candidate: &mut VerifiedSource,
    work: &mut Work<'_>,
) -> Result<ApplyOutcome, Error> {
    let destination = root.join(path.as_str());
    if destination.exists() {
        return Ok(ApplyOutcome::Conflict);
    }
    copy_exact(&destination, candidate, work)?;
    Ok(ApplyOutcome::Saved)
}

fn copy_exact(
    destination: &Path,
    source: &mut VerifiedSource,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let parent = destination.parent().ok_or(Error::Invalid)?;
    fs::create_dir_all(parent).map_err(|_| Error::Io)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| Error::Io)?;
    source.copy_to(&mut file, work)?;
    file.sync_all().map_err(|_| Error::Io)?;
    drop(file);
    let bytes = fs::read(destination).map_err(|_| Error::Io)?;
    if bytes.len() as u64 != source.len() || sha256_hex(&bytes) != source.digest().as_str() {
        return Err(Error::Drift);
    }
    Ok(())
}

struct NoGit {
    sink: PathBuf,
}

impl GitTransactions for NoGit {
    fn admit_inert_bundle(
        &mut self,
        operation_id: &str,
        bundle: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<GitAdmit, Error> {
        copy_exact(&self.sink.join(operation_id), bundle, work)?;
        Ok(GitAdmit::Inert)
    }
}

struct CopyGit {
    destination: PathBuf,
}

impl GitTransactions for CopyGit {
    fn admit_inert_bundle(
        &mut self,
        _operation_id: &str,
        bundle: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<GitAdmit, Error> {
        copy_exact(&self.destination, bundle, work)?;
        Ok(GitAdmit::Inert)
    }
}

fn bounds() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 64,
            max_bytes: 1 << 20,
            max_millis: 5_000,
            max_memory_bytes: 8 << 20,
        },
        quotas: Quotas {
            max_archive_bytes: 1 << 20,
            max_entries: 64,
            max_entry_expanded: 64 << 10,
            max_total_expanded: 256 << 10,
            max_ratio: 1_000,
            max_path_bytes: 1_024,
            max_depth: 16,
        },
    }
}

fn make_work<'a>(bounds: &'a Bounds, holding: &'a mut Holding) -> Work<'a> {
    static CANCEL: AtomicBool = AtomicBool::new(false);
    Work::new(bounds, holding, &CANCEL).expect("work")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn extra_field(id: u16, values: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + values.len() * 8);
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(values.len() as u16 * 8).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn stored_entry(name: &str, data: Vec<u8>) -> RawEntry {
    raw_entry(name, data, 0)
}

fn deflated_entry(name: &str, data: Vec<u8>) -> RawEntry {
    raw_entry(name, data, 8)
}

fn raw_entry(name: &str, data: Vec<u8>, method: u16) -> RawEntry {
    RawEntry {
        local_name: name.as_bytes().to_vec(),
        central_name: name.as_bytes().to_vec(),
        data,
        method,
        flags: 0x0800,
        version_made: 0x031e,
        version_needed: if method == 8 { 20 } else { 10 },
        local_extra: vec![],
        central_extra: vec![],
        external_attrs: 0o100644u32 << 16,
        disk_start: 0,
        crc: None,
        local_sizes: None,
        central_sizes: None,
        central_offset: None,
        force_zip64: false,
    }
}

#[derive(Clone)]
struct RawEntry {
    local_name: Vec<u8>,
    central_name: Vec<u8>,
    data: Vec<u8>,
    method: u16,
    flags: u16,
    version_made: u16,
    version_needed: u16,
    local_extra: Vec<u8>,
    central_extra: Vec<u8>,
    external_attrs: u32,
    disk_start: u16,
    crc: Option<u32>,
    local_sizes: Option<(u32, u32)>,
    central_sizes: Option<(u32, u32)>,
    central_offset: Option<u32>,
    force_zip64: bool,
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_raw_zip(entries: &[RawEntry], disk: u16) -> Vec<u8> {
    struct Meta {
        central_name: Vec<u8>,
        central_extra: Vec<u8>,
        method: u16,
        flags: u16,
        crc: u32,
        compressed: u32,
        expanded: u32,
        payload_len: u32,
        offset: u32,
        version_made: u16,
        version_needed: u16,
        external_attrs: u32,
        disk_start: u16,
    }

    let mut out = Vec::new();
    let mut metas = Vec::with_capacity(entries.len());
    for entry in entries {
        let payload = if entry.method == 8 {
            deflate(&entry.data)
        } else {
            entry.data.clone()
        };
        let crc = entry.crc.unwrap_or_else(|| crc32fast::hash(&entry.data));
        let offset = out.len() as u32;
        let mut local_extra = entry.local_extra.clone();
        let (compressed, expanded) = if entry.force_zip64 {
            local_extra.extend(extra_field(
                1,
                &[entry.data.len() as u64, payload.len() as u64],
            ));
            (u32::MAX, u32::MAX)
        } else {
            entry
                .local_sizes
                .unwrap_or((payload.len() as u32, entry.data.len() as u32))
        };
        let local_name = &entry.local_name;

        put_u32(&mut out, 0x0403_4b50);
        put_u16(&mut out, entry.version_needed);
        put_u16(&mut out, entry.flags);
        put_u16(&mut out, entry.method);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0x0021);
        put_u32(&mut out, crc);
        put_u32(&mut out, compressed);
        put_u32(&mut out, expanded);
        put_u16(&mut out, local_name.len() as u16);
        put_u16(&mut out, local_extra.len() as u16);
        out.extend_from_slice(local_name);
        out.extend_from_slice(&local_extra);
        out.extend_from_slice(&payload);

        metas.push(Meta {
            central_name: entry.central_name.clone(),
            central_extra: entry.central_extra.clone(),
            method: entry.method,
            flags: entry.flags,
            crc,
            compressed,
            expanded,
            payload_len: payload.len() as u32,
            offset,
            version_made: entry.version_made,
            version_needed: entry.version_needed,
            external_attrs: entry.external_attrs,
            disk_start: entry.disk_start,
        });
    }

    let central_offset = out.len() as u32;
    for (meta, entry) in metas.iter_mut().zip(entries) {
        let mut central_extra = std::mem::take(&mut meta.central_extra);
        let (compressed, expanded, offset) = if entry.force_zip64 {
            let offset = entry.central_offset.unwrap_or(meta.offset);
            central_extra.extend(extra_field(
                1,
                &[
                    entry.data.len() as u64,
                    meta.payload_len as u64,
                    offset as u64,
                ],
            ));
            (u32::MAX, u32::MAX, u32::MAX)
        } else {
            let (compressed, expanded) = entry
                .central_sizes
                .unwrap_or((meta.compressed, meta.expanded));
            (
                compressed,
                expanded,
                entry.central_offset.unwrap_or(meta.offset),
            )
        };

        put_u32(&mut out, 0x0201_4b50);
        put_u16(&mut out, meta.version_made);
        put_u16(&mut out, meta.version_needed);
        put_u16(&mut out, meta.flags);
        put_u16(&mut out, meta.method);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0x0021);
        put_u32(&mut out, meta.crc);
        put_u32(&mut out, compressed);
        put_u32(&mut out, expanded);
        put_u16(&mut out, meta.central_name.len() as u16);
        put_u16(&mut out, central_extra.len() as u16);
        put_u16(&mut out, 0);
        put_u16(&mut out, meta.disk_start);
        put_u16(&mut out, 0);
        put_u32(&mut out, meta.external_attrs);
        put_u32(&mut out, offset);
        out.extend_from_slice(&meta.central_name);
        out.extend_from_slice(&central_extra);
    }
    let central_size = out.len() as u32 - central_offset;

    put_u32(&mut out, 0x0605_4b50);
    put_u16(&mut out, disk);
    put_u16(&mut out, disk);
    put_u16(&mut out, entries.len() as u16);
    put_u16(&mut out, entries.len() as u16);
    put_u32(&mut out, central_size);
    put_u32(&mut out, central_offset);
    put_u16(&mut out, 0);
    out
}

fn patch_zip64_local_uncompressed(bytes: &mut [u8], name: &str, value: u64) -> bool {
    for pos in 0..bytes.len().saturating_sub(30) {
        if u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) != 0x0403_4b50 {
            continue;
        }
        let name_len = u16::from_le_bytes(bytes[pos + 26..pos + 28].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(bytes[pos + 28..pos + 30].try_into().unwrap()) as usize;
        let name_start = pos + 30;
        if bytes[name_start..name_start + name_len] != *name.as_bytes() {
            continue;
        }
        let extra_start = name_start + name_len;
        let mut cursor = extra_start;
        while cursor + 4 <= extra_start + extra_len {
            let id = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
            let size =
                u16::from_le_bytes(bytes[cursor + 2..cursor + 4].try_into().unwrap()) as usize;
            if id == 1 {
                bytes[cursor + 4..cursor + 12].copy_from_slice(&value.to_le_bytes());
                return true;
            }
            cursor += 4 + size;
        }
    }
    false
}

fn manifest_entry(manifest: &[u8]) -> RawEntry {
    stored_entry("manifest.json", manifest.to_vec())
}

fn entry_json(path: &str, data: &[u8], method: u16, role: &str) -> Value {
    json!({
        "method": if method == 8 { "deflate" } else { "stored" },
        "mount_id": null,
        "path": path,
        "role": role,
        "sha256": sha256_hex(data),
        "size_bytes": data.len().to_string(),
        "source_id": null,
    })
}

fn manifest_bytes(
    entries: &[(&str, &[u8], u16)],
    required_features: &[&str],
    version: u32,
) -> Vec<u8> {
    manifest_bytes_with_format(
        "omni.knowledge-package",
        entries,
        required_features,
        version,
    )
}

fn manifest_bytes_with_format(
    format: &str,
    entries: &[(&str, &[u8], u16)],
    required_features: &[&str],
    version: u32,
) -> Vec<u8> {
    let entries: Vec<Value> = entries
        .iter()
        .map(|(path, data, method)| entry_json(path, data, *method, "core"))
        .collect();
    let document = json!({
        "capture_window": {
            "end": "2026-01-02T00:00:00Z",
            "start": "2026-01-01T00:00:00Z",
        },
        "coverage": {
            "indexes": "none",
            "omitted": "0",
            "pending": "0",
            "selected": entries.len().to_string(),
            "unavailable": "0",
            "unverified": "0",
        },
        "entries": entries,
        "external_references": [],
        "format": format,
        "history": null,
        "indexes": null,
        "journal": null,
        "library_id": "library",
        "mounts": [],
        "name": "fixture",
        "preset": "frozen",
        "required_features": required_features,
        "text_root": "pages",
        "version": version,
    });
    serde_json::to_vec(&document).unwrap()
}

fn raw_fixture(data: &[u8]) -> Vec<u8> {
    let manifest = manifest_bytes(&[("pages/index.md", data, 8)], &[], 1);
    write_raw_zip(
        &[
            manifest_entry(&manifest),
            deflated_entry("pages/index.md", data.to_vec()),
        ],
        0,
    )
}

fn one_entry_archive(entry: RawEntry) -> Vec<u8> {
    let data = entry.data.clone();
    let manifest = manifest_bytes(&[("pages/index.md", &data, entry.method)], &[], 1);
    write_raw_zip(&[manifest_entry(&manifest), entry], 0)
}

fn assert_error(path: &Path, bytes: &[u8], expected: Error) {
    let result = inspect_bytes(path, bytes, &[], &bounds());
    assert_eq!(result.err(), Some(expected));
}

fn tree_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&path).unwrap(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn inspect_bytes(
    path: &Path,
    bytes: &[u8],
    existing: &[RelativePath],
    bounds: &Bounds,
) -> Result<knowledge_package::Preview, Error> {
    fs::write(path, bytes).unwrap();
    let mut archive = fs::File::open(path).unwrap();
    let mut lease = Lease::call();
    let mut work = make_work(bounds, &mut lease.holding);
    inspect(&mut archive, &mut work, existing)
}

fn capture_fixture(root: &Path) -> knowledge_package::Captured {
    let destination = root.join("fixture.zip");
    capture_fixture_to(root, &destination)
}

fn capture_fixture_to(root: &Path, destination: &Path) -> knowledge_package::Captured {
    let private = root.join("private");
    fs::create_dir_all(&private).unwrap();
    let mut bytes = Cursor::new(b"hello from the package\n".to_vec());
    let mut inputs = vec![Input {
        path: RelativePath::new("pages/index.md").unwrap(),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut bytes,
    }];
    let mut description = Capture {
        operation_id: "op",
        preset: Preset::Frozen,
        library_id: "library",
        name: "fixture",
        text_root: "pages",
        window: Window {
            start: "2026-01-01T00:00:00Z".to_owned(),
            end: "2026-01-02T00:00:00Z".to_owned(),
        },
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: false,
        required_features: vec![],
        private_dir: &private,
        destination,
    };
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    capture(&mut description, &mut inputs, &mut work).unwrap()
}

fn capture_history_fixture(root: &Path) -> knowledge_package::Captured {
    let private = root.join("history-capture-private");
    fs::create_dir_all(&private).unwrap();
    let destination = root.join("history-capture.zip");
    let mut core = Cursor::new(b"hello from the package\n".to_vec());
    let mut bundle = Cursor::new(b"inert bundle bytes\n".to_vec());
    let mut inputs = vec![
        Input {
            path: RelativePath::new("pages/index.md").unwrap(),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut core,
        },
        Input {
            path: RelativePath::new("history/bundle").unwrap(),
            role: Role::History,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut bundle,
        },
    ];
    let mut description = Capture {
        operation_id: "history-capture",
        preset: Preset::Backup,
        library_id: "library",
        name: "fixture",
        text_root: "pages",
        window: Window {
            start: "2026-01-01T00:00:00Z".to_owned(),
            end: "2026-01-02T00:00:00Z".to_owned(),
        },
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: false,
        required_features: vec![],
        private_dir: &private,
        destination: &destination,
    };
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    capture(&mut description, &mut inputs, &mut work).unwrap()
}

fn stage_fixture(root: &Path, private_name: &str, operation_id: &str) -> knowledge_package::Staged {
    let captured = capture_fixture(root);
    let private = root.join(private_name);
    fs::create_dir_all(&private).unwrap();
    let mut archive = fs::File::open(&captured.private_path).unwrap();
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    stage(&mut archive, &mut work, &private, operation_id, &[]).unwrap()
}

#[test]
fn capture_inspect_and_stage_round_trip_actual_files() {
    let _gate = gate();
    let dir = TempDir::new();
    let captured = capture_fixture(dir.path());
    assert!(captured.linked);
    assert_eq!(
        captured.installed.as_deref(),
        Some(dir.path().join("fixture.zip").as_path())
    );
    assert_eq!(captured.snapshot_id.as_str().len(), 64);

    let mut archive = fs::File::open(&captured.private_path).unwrap();
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let preview = inspect(&mut archive, &mut work, &[]).unwrap();
    assert_eq!(preview.snapshot_id.as_str(), captured.snapshot_id.as_str());
    assert_eq!(
        preview.integrity,
        knowledge_package::Integrity::AllEntriesVerified
    );
    assert_eq!(preview.signature, knowledge_package::Signature::Unsigned);
    assert!(preview.installable);
    assert_eq!(preview.manifest.entries.len(), 1);
    let entry = &preview.manifest.entries[0];
    assert_eq!(entry.path.as_str(), "pages/index.md");
    assert_eq!(entry.size_bytes, 23);
    assert_eq!(
        entry.sha256.as_str(),
        sha256_hex(b"hello from the package\n")
    );
    assert_eq!(entry.role, Role::Core);

    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("sentinel"), b"untouched").unwrap();
    let stage_private = dir.path().join("stage-private");
    fs::create_dir_all(&stage_private).unwrap();
    let mut archive = fs::File::open(&captured.private_path).unwrap();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let staged = stage(&mut archive, &mut work, &stage_private, "stage-op", &[]).unwrap();
    assert_eq!(staged.snapshot_id.as_str(), captured.snapshot_id.as_str());
    assert!(staged.installable);
    assert!(staged.conflicts.is_empty());
    assert_eq!(staged.files.len(), 1);
    assert_eq!(staged.files[0].path.as_str(), "pages/index.md");
    assert_eq!(
        staged.files[0].digest.as_str(),
        sha256_hex(b"hello from the package\n")
    );
    assert_eq!(
        fs::read(&staged.files[0].staged).unwrap(),
        b"hello from the package\n"
    );
    assert_eq!(fs::read(home.join("sentinel")).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&home).unwrap().count(), 1);
}

#[test]
fn inspect_reports_case_insensitive_conflict_without_writing_home() {
    let _gate = gate();
    let dir = TempDir::new();
    let captured = capture_fixture(dir.path());
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("sentinel"), b"untouched").unwrap();
    let existing = [RelativePath::new("pages/INDEX.MD").unwrap()];
    let mut archive = fs::File::open(&captured.private_path).unwrap();
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let preview = inspect(&mut archive, &mut work, &existing).unwrap();
    assert!(
        preview
            .conflicts
            .iter()
            .any(|path| path.as_str().eq_ignore_ascii_case("pages/index.md"))
    );
    assert_eq!(fs::read(home.join("sentinel")).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&home).unwrap().count(), 1);
}

#[test]
fn capture_links_only_an_absent_destination() {
    let _gate = gate();
    let dir = TempDir::new();
    let destination = dir.path().join("existing.zip");
    fs::write(&destination, b"previous bytes").unwrap();
    let captured = capture_fixture_to(dir.path(), &destination);
    assert!(!captured.linked);
    assert!(captured.installed.is_none());
    assert_eq!(fs::read(&destination).unwrap(), b"previous bytes");
    assert!(captured.private_path.exists());
    assert_eq!(captured.snapshot_id.as_str().len(), 64);
}

#[test]
fn rejects_traversal_absolute_backslash_and_dot_entry_names() {
    let _gate = gate();
    let dir = TempDir::new();
    let cases = [
        "../escape.md",
        "/absolute.md",
        "pages\\\\..\\\\escape.md",
        "pages/./index.md",
    ];
    for (index, name) in cases.into_iter().enumerate() {
        let bytes = one_entry_archive(deflated_entry(name, b"payload\n".to_vec()));
        let path = dir.path().join(format!("path-{index}.zip"));
        let result = inspect_bytes(&path, &bytes, &[], &bounds());
        assert_eq!(result.err(), Some(Error::Hostile), "name {name:?}");
    }
}

#[test]
fn rejects_drive_unc_empty_segment_and_nul_names() {
    let _gate = gate();
    let dir = TempDir::new();
    let cases = [
        "C:/escape.md",
        "\\\\server\\share\\escape.md",
        "pages//index.md",
        "pages/\0index.md",
    ];
    for (index, name) in cases.into_iter().enumerate() {
        let bytes = one_entry_archive(deflated_entry(name, b"payload\n".to_vec()));
        let path = dir.path().join(format!("absolute-{index}.zip"));
        assert!(
            inspect_bytes(&path, &bytes, &[], &bounds()).is_err(),
            "name {name:?}"
        );
    }
}

#[test]
fn rejects_duplicate_payload_entries() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = b"duplicate\n";
    let manifest = manifest_bytes(&[("pages/index.md", data, 0)], &[], 1);
    let bytes = write_raw_zip(
        &[
            manifest_entry(&manifest),
            stored_entry("pages/index.md", data.to_vec()),
            stored_entry("pages/index.md", data.to_vec()),
        ],
        0,
    );
    assert_error(&dir.path().join("duplicate.zip"), &bytes, Error::Hostile);
}

#[test]
fn rejects_symlink_and_device_entries() {
    let _gate = gate();
    let dir = TempDir::new();
    for (index, mode) in [0o120777u32, 0o020644u32].into_iter().enumerate() {
        let mut entry = stored_entry("pages/index.md", b"special\n".to_vec());
        entry.external_attrs = mode << 16;
        let bytes = one_entry_archive(entry);
        let path = dir.path().join(format!("special-{index}.zip"));
        assert_error(&path, &bytes, Error::Hostile);
    }
}

#[test]
fn rejects_encrypted_entry() {
    let _gate = gate();
    let dir = TempDir::new();
    let mut entry = deflated_entry("pages/index.md", b"encrypted\n".to_vec());
    entry.flags = 0x0801;
    let bytes = one_entry_archive(entry);
    assert_error(&dir.path().join("encrypted.zip"), &bytes, Error::Hostile);
}

#[test]
fn rejects_multi_disk_archive() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = b"multi disk\n";
    let manifest = manifest_bytes(&[("pages/index.md", data, 0)], &[], 1);
    let mut manifest_entry = manifest_entry(&manifest);
    let mut data_entry = stored_entry("pages/index.md", data.to_vec());
    manifest_entry.disk_start = 1;
    data_entry.disk_start = 1;
    let bytes = write_raw_zip(&[manifest_entry, data_entry], 1);
    assert_error(&dir.path().join("multi-disk.zip"), &bytes, Error::Hostile);
}

#[test]
fn rejects_local_and_central_header_mismatch() {
    let _gate = gate();
    let dir = TempDir::new();
    let mut entry = deflated_entry("pages/index.md", b"mismatch\n".to_vec());
    entry.central_name = b"pages/other.md".to_vec();
    let bytes = one_entry_archive(entry);
    assert_error(
        &dir.path().join("mismatch.zip"),
        &bytes,
        Error::Disagreement,
    );
}

#[test]
fn rejects_bad_crc() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = b"crc data\n";
    let mut entry = deflated_entry("pages/index.md", data.to_vec());
    entry.crc = Some(crc32fast::hash(data) ^ 0xffff_ffff);
    let bytes = one_entry_archive(entry);
    assert_error(&dir.path().join("bad-crc.zip"), &bytes, Error::Checksum);
}

#[test]
fn rejects_truncated_archive() {
    let _gate = gate();
    let dir = TempDir::new();
    let mut bytes = raw_fixture(b"truncated\n");
    bytes.truncate(bytes.len() - 1);
    assert_error(&dir.path().join("truncated.zip"), &bytes, Error::Truncated);
}

#[test]
fn rejects_unknown_required_feature_and_format_version() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = b"features\n";

    let manifest = manifest_bytes(&[("pages/index.md", data, 8)], &["future.index"], 1);
    let bytes = write_raw_zip(
        &[
            manifest_entry(&manifest),
            deflated_entry("pages/index.md", data.to_vec()),
        ],
        0,
    );
    let preview = inspect_bytes(&dir.path().join("feature.zip"), &bytes, &[], &bounds()).unwrap();
    assert!(!preview.installable);

    let manifest = manifest_bytes_with_format(
        "unknown.package-format",
        &[("pages/index.md", data, 8)],
        &[],
        1,
    );
    let bytes = write_raw_zip(
        &[
            manifest_entry(&manifest),
            deflated_entry("pages/index.md", data.to_vec()),
        ],
        0,
    );
    assert_error(
        &dir.path().join("unknown-format.zip"),
        &bytes,
        Error::Unsupported,
    );

    let manifest = manifest_bytes(&[("pages/index.md", data, 8)], &[], 2);
    let bytes = write_raw_zip(
        &[
            manifest_entry(&manifest),
            deflated_entry("pages/index.md", data.to_vec()),
        ],
        0,
    );
    assert_error(&dir.path().join("version.zip"), &bytes, Error::Unsupported);
}

#[test]
fn rejects_ratio_above_caller_bound() {
    let _gate = gate();
    let dir = TempDir::new();
    let bytes = raw_fixture(&vec![0u8; 32 << 10]);
    let mut tight = bounds();
    tight.quotas.max_ratio = 10;
    let result = inspect_bytes(&dir.path().join("ratio.zip"), &bytes, &[], &tight);
    assert_eq!(result.err(), Some(Error::Ratio));
}

#[test]
fn rejects_declared_size_below_actual_expansion() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = vec![0xab; 4 << 10];
    let compressed = deflate(&data).len() as u32;
    let mut entry = deflated_entry("pages/index.md", data);
    entry.local_sizes = Some((compressed, 1));
    entry.central_sizes = Some((compressed, 1));
    let bytes = one_entry_archive(entry);
    assert_error(
        &dir.path().join("declared.zip"),
        &bytes,
        Error::Disagreement,
    );
}

#[test]
fn accepts_bounded_zip64_entry_and_rejects_header_disagreement() {
    let _gate = gate();
    let dir = TempDir::new();
    let data = b"zip64 payload\n";
    let mut entry = deflated_entry("pages/index.md", data.to_vec());
    entry.force_zip64 = true;
    entry.version_needed = 45;
    entry.version_made = 0x032d;
    let mut bytes = one_entry_archive(entry);
    let preview = inspect_bytes(&dir.path().join("zip64.zip"), &bytes, &[], &bounds()).unwrap();
    assert_eq!(preview.manifest.entries.len(), 1);

    assert!(patch_zip64_local_uncompressed(
        &mut bytes,
        "pages/index.md",
        1
    ));
    assert_error(
        &dir.path().join("zip64-bad.zip"),
        &bytes,
        Error::Disagreement,
    );
}

#[test]
fn rejects_overlapping_entry_extents() {
    let _gate = gate();
    let dir = TempDir::new();
    let a = b"A";
    let b = b"B";
    let manifest = manifest_bytes(&[("pages/a.md", a, 0), ("pages/b.md", b, 0)], &[], 1);
    let mut first = stored_entry("pages/a.md", a.to_vec());
    first.local_sizes = Some((500, 500));
    first.central_sizes = Some((500, 500));
    let second = stored_entry("pages/b.md", b.to_vec());
    let bytes = write_raw_zip(&[manifest_entry(&manifest), first, second], 0);
    assert_error(&dir.path().join("overlap.zip"), &bytes, Error::Hostile);
}

#[test]
fn stage_keeps_previous_generation_on_refusal() {
    let _gate = gate();
    let dir = TempDir::new();
    let private = dir.path().join("stage-private");
    fs::create_dir_all(&private).unwrap();
    let good_path = dir.path().join("good.zip");
    let good = raw_fixture(b"previous\n");
    fs::write(&good_path, &good).unwrap();
    let mut archive = fs::File::open(&good_path).unwrap();
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let staged = stage(&mut archive, &mut work, &private, "previous", &[]).unwrap();
    let before = tree_snapshot(&staged.generation);

    let mut bad_entry = deflated_entry("pages/index.md", b"previous\n".to_vec());
    bad_entry.crc = Some(crc32fast::hash(b"previous\n") ^ 0xffff_ffff);
    let bad = one_entry_archive(bad_entry);
    let bad_path = dir.path().join("bad.zip");
    fs::write(&bad_path, &bad).unwrap();
    let mut archive = fs::File::open(&bad_path).unwrap();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let refused = stage(&mut archive, &mut work, &private, "current", &[]);
    assert!(refused.is_err());
    assert_eq!(tree_snapshot(&staged.generation), before);
}

#[test]
fn apply_measures_owner_state_and_preserves_conflict_file() {
    let _gate = gate();
    let dir = TempDir::new();
    let staged = stage_fixture(dir.path(), "apply-private", "apply-op");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let mut install = FsApply { root: home.clone() };
    let mut git = NoGit {
        sink: dir.path().join("git-sink"),
    };
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let result = apply(&staged, &mut install, &mut git, &mut work).unwrap();
    assert_eq!(result.state, ApplyState::Completed);
    assert_eq!(result.applied.len(), 1);
    assert_eq!(
        fs::read(home.join("pages/index.md")).unwrap(),
        b"hello from the package\n"
    );

    let conflict_home = dir.path().join("conflict-home");
    fs::create_dir_all(conflict_home.join("pages")).unwrap();
    fs::write(conflict_home.join("pages/index.md"), b"caller bytes").unwrap();
    let mut install = FsApply {
        root: conflict_home.clone(),
    };
    let mut git = NoGit {
        sink: dir.path().join("conflict-git-sink"),
    };
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let result = apply(&staged, &mut install, &mut git, &mut work).unwrap();
    assert_eq!(result.state, ApplyState::Conflict);
    assert!(
        result
            .conflicts
            .iter()
            .any(|path| path.as_str() == "pages/index.md")
    );
    assert_eq!(
        fs::read(conflict_home.join("pages/index.md")).unwrap(),
        b"caller bytes"
    );
}

#[test]
fn apply_hands_staged_history_bundle_to_git_owner() {
    let _gate = gate();
    let dir = TempDir::new();
    let core = b"hello from the package\n";
    let captured = capture_history_fixture(dir.path());
    let private = dir.path().join("history-private");
    fs::create_dir_all(&private).unwrap();
    let mut archive = fs::File::open(&captured.private_path).unwrap();
    let bounds = bounds();
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let staged = stage(&mut archive, &mut work, &private, "history-op", &[]).unwrap();
    let bundle_path = staged.history.as_ref().expect("history bundle staged");

    let home = dir.path().join("history-home");
    fs::create_dir_all(&home).unwrap();
    let admitted = dir.path().join("admitted-bundle");
    let mut install = FsApply { root: home.clone() };
    let mut git = CopyGit {
        destination: admitted.clone(),
    };
    let mut lease = Lease::call();
    let mut work = make_work(&bounds, &mut lease.holding);
    let result = apply(&staged, &mut install, &mut git, &mut work).unwrap();
    assert_eq!(result.state, ApplyState::Completed);
    assert_eq!(fs::read(admitted).unwrap(), fs::read(bundle_path).unwrap());
    assert_eq!(fs::read(home.join("pages/index.md")).unwrap(), core);
}

#[test]
fn admission_refusal_returns_waiting_without_home_effect() {
    let _gate = gate();
    let dir = TempDir::new();
    let archive_path = dir.path().join("admission.zip");
    fs::write(&archive_path, raw_fixture(&vec![0u8; 8 << 10])).unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("sentinel"), b"untouched").unwrap();

    let account = ensure_account();
    let mut worker = match account
        .reserve(
            Reservation {
                memory_bytes: 1,
                temporary_bytes: 1,
            },
            Origin::Startup,
        )
        .expect("small reserve")
    {
        Decision::Ready(holding) => Lease { holding },
        Decision::Waiting(waiting) => panic!("small reserve waited: {waiting:?}"),
    };
    let status = account.status().expect("admission status");
    let _blocker = match account
        .reserve(
            Reservation {
                memory_bytes: status.memory_available,
                temporary_bytes: status.storage_available,
            },
            Origin::Startup,
        )
        .expect("exhaust admission")
    {
        Decision::Ready(holding) => Lease { holding },
        Decision::Waiting(waiting) => panic!("exhaustion reserve waited: {waiting:?}"),
    };
    let mut archive = fs::File::open(&archive_path).unwrap();
    let bounds = bounds();
    let mut work = make_work(&bounds, &mut worker.holding);
    let result = inspect(&mut archive, &mut work, &[]);
    assert_eq!(result.err(), Some(Error::Waiting));
    assert_eq!(fs::read(home.join("sentinel")).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&home).unwrap().count(), 1);
}

#[test]
fn bounded_raw_archive_is_accepted() {
    let _gate = gate();
    let dir = TempDir::new();
    let bytes = raw_fixture(b"raw fixture bytes\n");
    let preview = inspect_bytes(&dir.path().join("raw.zip"), &bytes, &[], &bounds()).unwrap();
    assert_eq!(preview.manifest.entries.len(), 1);
    assert_eq!(preview.manifest.entries[0].path.as_str(), "pages/index.md");
}
