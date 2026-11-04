// Shared public-cache premise. Included by the contract tests and the
// single-threaded public consumer. This is not a provider, an authorizer,
// or a cache implementation.
//
// Digests are SHA-256 of the real fixture bytes, checked against the published
// vectors below before any cache call. Premise is the one trusted current
// root and source-mapping boundary the cache asks for. Supplying it is not
// provider discovery, M6 authentication, or exposure approval.

use knowledge_core::cache::{
    Cache, CacheContext, CacheError, DirectoryIdentity, Limits, Locality, PinnedObject,
    ProcessorIdentity, Publication, RootFacts, SecurityDomain, SourceAdmission,
};
use knowledge_core::data::ContentDigest;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
#[cfg(not(cache_public_consumer))]
use std::process::{Command, Stdio};
#[cfg(not(cache_public_consumer))]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CANARY: &str = "SENTINEL_cache_6c2e91";
pub const LIBRARY: &str = "library-1";
pub const SOURCE: &str = "source-1";

pub fn assert_sha256_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_ne!(sha256_hex(b"abc"), sha256_hex(b""));
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// Fixture digests use locked `sha2`, not a second SHA-256 and not the cache fingerprint.
pub fn sha256_hex(data: &[u8]) -> String {
    #[cfg(cache_public_consumer)]
    {
        use sha2::{Digest, Sha256};
        hex_encode(&Sha256::digest(data))
    }
    #[cfg(not(cache_public_consumer))]
    {
        external_sha256(data)
    }
}

#[cfg(not(cache_public_consumer))]
fn external_sha256(data: &[u8]) -> String {
    let bin = consumer_executable();
    let mut child = Command::new(&bin)
        .arg("sha256")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sha2 helper");
    child
        .stdin
        .take()
        .expect("sha2 stdin")
        .write_all(data)
        .expect("write sha2 stdin");
    let output = child.wait_with_output().expect("sha2 helper");
    assert!(
        output.status.success(),
        "sha2 helper failed\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = String::from_utf8(output.stdout).expect("sha2 hex");
    line.trim().to_string()
}

#[cfg(not(cache_public_consumer))]
pub fn consumer_executable() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(build_consumer).clone()
}

#[cfg(not(cache_public_consumer))]
fn build_consumer() -> PathBuf {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = package.join("tests/cache/consumer/Cargo.toml");
    let repo = package
        .parent()
        .and_then(|parent| parent.parent())
        .expect("repository")
        .to_path_buf();
    let scratch = repo.parent().expect("scratch").join("target-consumer");
    let target = scratch.join("target");
    fs::create_dir_all(&target).expect("consumer target");
    let lock = scratch.join("build.lock");
    fs::File::create(&lock).expect("consumer build lock");
    let output = Command::new("flock")
        .arg(&lock)
        .arg("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--locked")
        .arg("--offline")
        .arg("-j")
        .arg("2")
        .arg("--message-format=json")
        .env("CARGO_TARGET_DIR", &target)
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            "--cfg\u{1f}cache_public_consumer",
        )
        .env_remove("RUSTFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("CARGO_MAKEFLAGS")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("cargo build consumer");
    assert!(
        output.status.success(),
        "consumer build failed\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let exe = cargo_executable(&String::from_utf8_lossy(&output.stdout));
    assert!(exe.is_file(), "cargo did not record an executable");
    assert!(
        dep_info_tracks_cache_source(&target),
        "consumer build did not track knowledge-core cache source"
    );
    let head = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git head");
    assert!(head.status.success(), "git rev-parse failed");
    let head = String::from_utf8(head.stdout)
        .expect("head")
        .trim()
        .to_string();
    let exe_hash = hash_with_consumer(&exe, &exe);
    let identity = scratch.join("helper-identity.txt");
    fs::write(
        &identity,
        format!(
            "head {head}\nexecutable {}\nsha256 {exe_hash}\ntracks cache.rs native.rs\n",
            exe.display()
        ),
    )
    .expect("identity");
    exe
}

#[cfg(not(cache_public_consumer))]
fn cargo_executable(stdout: &str) -> PathBuf {
    let mut found = None;
    for line in stdout.lines() {
        let Some(executable) = json_string_field(line, "executable") else {
            continue;
        };
        if executable.ends_with("cache-public-consumer") {
            found = Some(PathBuf::from(executable));
        }
    }
    found.expect("compiler-artifact executable")
}

#[cfg(not(cache_public_consumer))]
fn json_string_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\":\"");
    let start = line.find(&key)? + key.len();
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

#[cfg(not(cache_public_consumer))]
fn dep_info_tracks_cache_source(target: &Path) -> bool {
    let mut cache_rs = false;
    let mut native_rs = false;
    let mut stack = vec![target.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("d") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            cache_rs |= text.contains("src/cache.rs");
            native_rs |= text.contains("src/cache/native.rs");
            if cache_rs && native_rs {
                return true;
            }
        }
    }
    false
}

#[cfg(not(cache_public_consumer))]
fn hash_with_consumer(bin: &Path, file: &Path) -> String {
    let bytes = fs::read(file).expect("hash input");
    let mut child = Command::new(bin)
        .arg("sha256")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("hash executable");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&bytes)
        .expect("write");
    let output = child.wait_with_output().expect("hash wait");
    assert!(output.status.success(), "hash helper failed");
    String::from_utf8(output.stdout)
        .expect("hash hex")
        .trim()
        .to_string()
}

pub fn digest_of(bytes: &[u8]) -> ContentDigest {
    ContentDigest::new(sha256_hex(bytes)).expect("lowercase sha256 spelling")
}

pub fn generous_limits() -> Limits {
    Limits {
        max_disk_bytes: 8 * 1024 * 1024,
        max_object_bytes: 1024 * 1024,
        max_read_bytes: 1024 * 1024,
        buffer_bytes: 4096,
        max_entries: 4096,
        max_identity_bytes: 256 * 1024,
        max_pins: 8,
    }
}

pub fn processor(option: &str, transcript: &[u8], annotation: &[u8]) -> ProcessorIdentity {
    let mut options = BTreeMap::new();
    options.insert("threshold".to_string(), option.to_string());
    let mut preprocessing = BTreeMap::new();
    preprocessing.insert("order".to_string(), "decode,segment".to_string());
    let mut dependencies = BTreeMap::new();
    dependencies.insert("corrected-transcript".to_string(), digest_of(transcript));
    dependencies.insert("embedding-annotation".to_string(), digest_of(annotation));
    ProcessorIdentity {
        code: digest_of(b"cache-contract-processor-code-v1"),
        version: "1".to_string(),
        options,
        model: Some(digest_of(b"cache-contract-model-v1")),
        package: Some(digest_of(b"cache-contract-package-v1")),
        space: Some("contract-space".to_string()),
        dimensions: Some(8),
        preprocessing,
        dependencies,
    }
}

pub fn variant_name(error: &CacheError) -> &'static str {
    match error {
        CacheError::Invalid => "Invalid",
        CacheError::UnsafeRoot => "UnsafeRoot",
        CacheError::Busy => "Busy",
        CacheError::Changed => "Changed",
        CacheError::Budget => "Budget",
        CacheError::Cancelled => "Cancelled",
        CacheError::Missing => "Missing",
        CacheError::Corrupt => "Corrupt",
        CacheError::Conflict => "Conflict",
        CacheError::InspectionRequired => "InspectionRequired",
        CacheError::Indeterminate => "Indeterminate",
        CacheError::Io => "Io",
    }
}

pub fn assert_input_free(error: &CacheError) {
    assert!(
        error.source().is_none(),
        "cache error exposes a source chain"
    );
    let rendered = error.to_string();
    assert!(!rendered.is_empty(), "cache diagnostic is empty");
    assert!(
        !rendered.contains(CANARY),
        "diagnostic echoed the canary: {rendered}"
    );
    let debug = format!("{error:?}");
    assert!(!debug.contains(CANARY), "debug echoed the canary: {debug}");
}

fn unique_name() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("omni-cache-{CANARY}-{}-{nanos}-{n}", std::process::id())
}

pub struct Lab {
    root: PathBuf,
    armed: bool,
}

impl Lab {
    pub fn create() -> Self {
        let root = std::env::temp_dir().join(unique_name());
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&root).expect("private lab");
        force_mode(&root, 0o700);
        Self { root, armed: true }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn finish(mut self) {
        self.armed = false;
        fs::remove_dir_all(&self.root).expect("remove private lab");
    }
}

impl Drop for Lab {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub fn force_mode(path: &Path, mode: u32) {
    let mut permissions = fs::symlink_metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("chmod");
    let actual = fs::symlink_metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        actual, mode,
        "filesystem did not keep mode {mode:o} on {path:?}"
    );
}

pub fn make_private_dir(path: &Path) {
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path).expect("mkdir");
    force_mode(path, 0o700);
}

pub fn write_private_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        make_private_dir(parent);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .expect("create file");
    file.write_all(bytes).expect("write file");
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    File { bytes: Vec<u8> },
    Symlink { target: PathBuf },
    Dir,
    Other,
}

pub fn snapshot(root: &Path) -> BTreeMap<PathBuf, Node> {
    let mut out = BTreeMap::new();
    if fs::symlink_metadata(root).is_err() {
        return out;
    }
    walk_snapshot(root, root, &mut out);
    out
}

fn walk_snapshot(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Node>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(root).expect("prefix").to_path_buf();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).unwrap_or_default();
            out.insert(relative, Node::Symlink { target });
        } else if metadata.is_dir() {
            out.insert(relative.clone(), Node::Dir);
            walk_snapshot(root, &path, out);
        } else if metadata.is_file() {
            let bytes = fs::read(&path).unwrap_or_default();
            out.insert(relative, Node::File { bytes });
        } else {
            out.insert(relative, Node::Other);
        }
    }
}

pub fn corrupt_regular_files_at_least(root: &Path, min_len: usize) -> usize {
    let before = snapshot(root);
    let mut changed = 0;
    for (relative, node) in &before {
        let Node::File { bytes } = node else {
            continue;
        };
        if bytes.len() < min_len || bytes.is_empty() {
            continue;
        }
        let path = root.join(relative);
        let original_mode = fs::symlink_metadata(&path)
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        let mut permissions = fs::symlink_metadata(&path).expect("meta").permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&path, permissions).expect("make writable");
        let mut updated = bytes.clone();
        let last = updated.len() - 1;
        updated[last] ^= 0xff;
        fs::write(&path, &updated).expect("corrupt");
        force_mode(&path, original_mode);
        let restored = fs::symlink_metadata(&path)
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            restored, original_mode,
            "corruption left {path:?} at mode {restored:o} instead of admitted {original_mode:o}"
        );
        changed += 1;
    }
    changed
}

struct Shared {
    cache_dir: PathBuf,
    cache_identity: DirectoryIdentity,
    locality: Locality,
    prohibited: Vec<PathBuf>,
    home: PathBuf,
    home_identity: DirectoryIdentity,
    content: ContentDigest,
    domain: SecurityDomain,
    revision: u64,
    library: String,
    source: String,
    processor: ProcessorIdentity,
    refuse: bool,
    resolves: Vec<(String, String)>,
}

/// Trusted current roots and source mapping for one disposable fixture.
/// Field writes are the test's explicit premises. They do not discover a
/// provider or grant service authority.
#[derive(Clone)]
pub struct Premise {
    shared: Arc<Mutex<Shared>>,
}

impl Premise {
    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().expect("premise lock")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.lock().cache_dir.clone()
    }

    pub fn home(&self) -> PathBuf {
        self.lock().home.clone()
    }

    pub fn binding(&self) -> (String, String, ProcessorIdentity) {
        let shared = self.lock();
        (
            shared.library.clone(),
            shared.source.clone(),
            shared.processor.clone(),
        )
    }

    pub fn content(&self) -> ContentDigest {
        self.lock().content.clone()
    }

    pub fn processor(&self) -> ProcessorIdentity {
        self.lock().processor.clone()
    }

    pub fn resolves(&self) -> Vec<(String, String)> {
        self.lock().resolves.clone()
    }

    pub fn clear_resolves(&self) {
        self.lock().resolves.clear();
    }

    pub fn set_revision(&self, revision: u64) {
        self.lock().revision = revision;
    }

    pub fn set_content(&self, content: ContentDigest) {
        self.lock().content = content;
    }

    pub fn set_domain(&self, domain: SecurityDomain) {
        self.lock().domain = domain;
    }

    pub fn set_processor(&self, processor: ProcessorIdentity) {
        self.lock().processor = processor;
    }

    pub fn set_home_identity(&self, identity: DirectoryIdentity) {
        self.lock().home_identity = identity;
    }

    pub fn set_home(&self, home: PathBuf) {
        self.lock().home = home;
    }

    pub fn set_library(&self, library: impl Into<String>) {
        self.lock().library = library.into();
    }

    pub fn set_source(&self, source: impl Into<String>) {
        self.lock().source = source.into();
    }

    pub fn set_refuse(&self, refuse: bool) {
        self.lock().refuse = refuse;
    }

    pub fn set_locality(&self, locality: Locality) {
        self.lock().locality = locality;
    }

    pub fn set_prohibited(&self, prohibited: Vec<PathBuf>) {
        self.lock().prohibited = prohibited;
    }

    pub fn prohibited(&self) -> Vec<PathBuf> {
        self.lock().prohibited.clone()
    }

    pub fn set_cache_identity(&self, identity: DirectoryIdentity) {
        self.lock().cache_identity = identity;
    }

    pub fn domain(&self) -> SecurityDomain {
        self.lock().domain
    }
}

impl CacheContext for Premise {
    fn roots(&self) -> Result<RootFacts, CacheError> {
        let shared = self.lock();
        Ok(RootFacts {
            cache_identity: shared.cache_identity,
            locality: shared.locality,
            prohibited_roots: shared.prohibited.clone(),
        })
    }

    fn resolve(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
    ) -> Result<SourceAdmission, CacheError> {
        let mut shared = self.lock();
        shared
            .resolves
            .push((library.to_string(), source.to_string()));
        if shared.refuse || library != shared.library || source != shared.source {
            return Err(CacheError::Invalid);
        }
        if processor != &shared.processor {
            return Err(CacheError::Changed);
        }
        Ok(SourceAdmission {
            home: shared.home.clone(),
            home_identity: shared.home_identity,
            content: shared.content.clone(),
            domain: shared.domain,
            revision: shared.revision,
        })
    }
}

pub struct World {
    pub lab: Lab,
    pub premise: Premise,
    pub original: Vec<u8>,
    pub derived: Vec<u8>,
}

impl World {
    pub fn create(derived: &[u8]) -> Self {
        assert_sha256_vectors();
        let original = b"original-asset-bytes-v1".to_vec();
        assert_ne!(original.as_slice(), derived);
        let transcript = b"corrected-transcript-v1".to_vec();
        let annotation = b"embedding-annotation-v1".to_vec();
        let lab = Lab::create();
        populate(lab.root(), &original, derived, &transcript, &annotation);
        let premise = premise_at(lab.root(), &original, &transcript, &annotation);
        Self {
            lab,
            premise,
            original,
            derived: derived.to_vec(),
        }
    }

    pub fn open(&self, limits: Limits) -> Result<Cache<Premise>, CacheError> {
        Cache::open(&self.premise.cache_dir(), self.premise.clone(), limits)
    }
}

pub struct Attached {
    pub premise: Premise,
    pub original: Vec<u8>,
    pub derived: Vec<u8>,
}

impl Attached {
    pub fn open(&self, limits: Limits) -> Result<Cache<Premise>, CacheError> {
        Cache::open(&self.premise.cache_dir(), self.premise.clone(), limits)
    }
}

pub fn attach(root: &Path) -> Attached {
    assert_sha256_vectors();
    let original = fs::read(root.join("home").join("original.bin")).expect("original fixture");
    let transcript =
        fs::read(root.join("home").join("transcript.bin")).expect("transcript fixture");
    let annotation =
        fs::read(root.join("home").join("annotation.bin")).expect("annotation fixture");
    let derived = fs::read(root.join("derived.bin")).expect("derived fixture");
    Attached {
        premise: premise_at(root, &original, &transcript, &annotation),
        original,
        derived,
    }
}

fn populate(root: &Path, original: &[u8], derived: &[u8], transcript: &[u8], annotation: &[u8]) {
    let cache = root.join("cache");
    let home = root.join("home");
    for dir in [
        &cache,
        &home,
        &root.join("mount"),
        &root.join("cloud"),
        &root.join("history"),
    ] {
        make_private_dir(dir);
    }
    write_private_file(&home.join("original.bin"), original);
    write_private_file(&home.join("transcript.bin"), transcript);
    write_private_file(&home.join("annotation.bin"), annotation);
    write_private_file(&root.join("derived.bin"), derived);
}

fn premise_at(root: &Path, original: &[u8], transcript: &[u8], annotation: &[u8]) -> Premise {
    let cache = root.join("cache");
    let home = root.join("home");
    Premise {
        shared: Arc::new(Mutex::new(Shared {
            cache_dir: absolute_dir(&cache),
            cache_identity: DirectoryIdentity::observe(&cache).expect("cache identity"),
            locality: Locality::Local,
            prohibited: vec![
                absolute_dir(&home),
                absolute_dir(&root.join("mount")),
                absolute_dir(&root.join("cloud")),
                absolute_dir(&root.join("history")),
            ],
            home: absolute_dir(&home),
            home_identity: DirectoryIdentity::observe(&home).expect("home identity"),
            content: digest_of(original),
            domain: SecurityDomain::Sensitive,
            revision: 1,
            library: LIBRARY.to_string(),
            source: SOURCE.to_string(),
            processor: processor("1.10", transcript, annotation),
            refuse: false,
            resolves: Vec::new(),
        })),
    }
}

fn absolute_dir(path: &Path) -> PathBuf {
    std::path::absolute(path).expect("absolute")
}

pub fn publish_bytes(
    cache: &Cache<Premise>,
    premise: &Premise,
    bytes: &[u8],
) -> Result<Publication, CacheError> {
    let (library, source, processor) = premise.binding();
    let mut input = Cursor::new(bytes);
    cache.publish(&library, &source, &processor, &mut input, || false)
}

pub fn pin_object(
    cache: &Cache<Premise>,
    premise: &Premise,
) -> Result<PinnedObject<Premise>, CacheError> {
    let (library, source, processor) = premise.binding();
    cache.pin(&library, &source, &processor)
}

pub fn read_exact(
    cache: &Cache<Premise>,
    premise: &Premise,
    len: usize,
) -> Result<Vec<u8>, CacheError> {
    let pin = pin_object(cache, premise)?;
    pin.read(0, len)
}

pub fn original_unchanged(world: &World) {
    let path = world.premise.home().join("original.bin");
    let bytes = fs::read(&path).expect("original");
    assert_eq!(bytes, world.original);
}

pub fn payload_sized_files(root: &Path, min_len: usize) -> usize {
    snapshot(root)
        .values()
        .filter(|node| matches!(node, Node::File { bytes } if bytes.len() >= min_len))
        .count()
}

pub fn read_to_vec(path: &Path) -> Vec<u8> {
    fs::read(path).expect("read")
}

pub fn file_is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn open_read(path: &Path) -> io::Result<File> {
    File::open(path)
}

/// `/proc` start token for one pid. Field 22 is starttime; the command field may contain spaces.
pub fn start_token(pid: u32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let end = stat.rfind(')')?;
    let starttime = stat[end + 1..].split_whitespace().nth(19)?;
    Some(format!("{pid}:{starttime}"))
}
