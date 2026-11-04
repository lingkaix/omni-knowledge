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
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CANARY: &str = "SENTINEL_cache_6c2e91";
pub const LIBRARY: &str = "library-1";
pub const SOURCE: &str = "source-1";

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

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

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = sha256(data);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).saturating_mul(8);
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in message.chunks(64) {
        let mut words = [0u32; 64];
        for (index, word) in chunk.as_chunks::<4>().0.iter().take(16).enumerate() {
            words[index] = u32::from_be_bytes(*word);
        }
        for index in 16..64 {
            let first = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let second = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(first)
                .wrapping_add(words[index - 7])
                .wrapping_add(second);
        }
        let mut a = hash[0];
        let mut b = hash[1];
        let mut c = hash[2];
        let mut d = hash[3];
        let mut e = hash[4];
        let mut f = hash[5];
        let mut g = hash[6];
        let mut h = hash[7];
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(SHA256_K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        hash[0] = hash[0].wrapping_add(a);
        hash[1] = hash[1].wrapping_add(b);
        hash[2] = hash[2].wrapping_add(c);
        hash[3] = hash[3].wrapping_add(d);
        hash[4] = hash[4].wrapping_add(e);
        hash[5] = hash[5].wrapping_add(f);
        hash[6] = hash[6].wrapping_add(g);
        hash[7] = hash[7].wrapping_add(h);
    }
    let mut out = [0u8; 32];
    for (index, value) in hash.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    out
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
        let mut permissions = fs::symlink_metadata(&path).expect("meta").permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&path, permissions).expect("make writable");
        let mut updated = bytes.clone();
        let last = updated.len() - 1;
        updated[last] ^= 0xff;
        fs::write(&path, &updated).expect("corrupt");
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
        let presented = processor
            .fingerprint(1024 * 1024)
            .map_err(|_| CacheError::Invalid)?;
        let expected = shared
            .processor
            .fingerprint(1024 * 1024)
            .map_err(|_| CacheError::Invalid)?;
        if presented != expected {
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
