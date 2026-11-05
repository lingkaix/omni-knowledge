//! Confined descriptor primitives; every install is exclusive and every displaced entry is retained.
use crate::{Error, Version};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, openat, statat};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
};

/// Bind an absolute directory without symlinks or ancestor fallback.
pub(crate) fn bind(path: &Path) -> Result<File, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let mut root = File::open("/").map_err(|_| Error::Io)?;
    for c in path.components() {
        match c {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = name.to_str().ok_or(Error::Invalid)?;
                root = directory(&root, name)?;
            }
            _ => return Err(Error::Invalid),
        }
    }
    Ok(root)
}
/// Open an ordinary directory, never follow a symlink.
pub(crate) fn directory(parent: &File, name: &str) -> Result<File, Error> {
    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| {
        if e == rustix::io::Errno::NOENT {
            Error::Missing
        } else {
            Error::Io
        }
    })
}
/// Ensure a selected private bookkeeping child, refusing a substituted object or foreign mode/owner.
pub(crate) fn private_child(parent: &File, name: &str) -> Result<File, Error> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(_) => return Err(Error::Io),
    }
    let f = directory(parent, name)?;
    let m = f.metadata().map_err(|_| Error::Io)?;
    if m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o7777 != 0o700 {
        return Err(Error::Context);
    }
    parent.sync_all().map_err(|_| Error::Io)?;
    Ok(f)
}
/// Create one operation namespace exclusively; preexisting data is never adopted implicitly.
pub(crate) fn new_child(parent: &File, name: &str) -> Result<Option<File>, Error> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) => {
            parent.sync_all().map_err(|_| Error::Io)?;
            Ok(Some(directory(parent, name)?))
        }
        Err(rustix::io::Errno::EXIST) => Ok(None),
        Err(_) => Err(Error::Io),
    }
}
/// Read root identity from the held native directory, not a path or data declaration.
pub(crate) fn identity(f: &File) -> Result<(u64, u64), Error> {
    let m = f.metadata().map_err(|_| Error::Io)?;
    if !m.is_dir() {
        return Err(Error::Context);
    }
    Ok((m.dev(), m.ino()))
}
/// Determine actual descriptor ancestry without path-prefix guesses or following symlinks.
pub(crate) fn within(child: &File, ancestor: &File) -> Result<bool, Error> {
    let target = identity(ancestor)?;
    let mut current = child.try_clone().map_err(|_| Error::Io)?;
    loop {
        let id = identity(&current)?;
        if id == target {
            return Ok(true);
        }
        let parent = directory(&current, "..")?;
        if identity(&parent)? == id {
            return Ok(false);
        }
        current = parent;
    }
}
/// Check current selected path still resolves to the captured root.
pub(crate) fn bound(path: &Path, held: &File) -> Result<(), Error> {
    if identity(&bind(path)?)? != identity(held)? {
        return Err(Error::Context);
    }
    Ok(())
}
/// Any marker entry, even malformed or a symlink, is a child-library boundary.
pub(crate) fn marker(dir: &File) -> Result<bool, Error> {
    match statat(dir, "omni-knowledge.json", AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(_) => Err(Error::Io),
    }
}
/// Resolve a selected parent without symlinks, child roots, cross-volume descent or ancestor fallback.
/// Missing parents can be created only after durable operation intent, when create is explicit.
pub(crate) fn parent(root: &File, path: &str, create: bool) -> Result<(File, String), Error> {
    let mut parts = path.split('/');
    let name = parts.next_back().ok_or(Error::Invalid)?;
    let dev = identity(root)?.0;
    let mut dir = root.try_clone().map_err(|_| Error::Io)?;
    for part in parts {
        let next = match directory(&dir, part) {
            Ok(f) => f,
            Err(Error::Missing) => {
                if !create {
                    return Err(Error::Missing);
                }
                match rustix::fs::mkdirat(&dir, part, Mode::from_raw_mode(0o700)) {
                    Ok(()) => {}
                    Err(rustix::io::Errno::EXIST) => {}
                    Err(_) => return Err(Error::Io),
                }
                dir.sync_all().map_err(|_| Error::Io)?;
                directory(&dir, part)?
            }
            Err(e) => return Err(e),
        };
        if identity(&next)?.0 != dev || marker(&next)? {
            return Err(Error::Context);
        }
        dir = next;
    }
    Ok((dir, name.to_owned()))
}
/// Open only a regular file and verify the checked name still identifies that descriptor.
pub(crate) fn regular(parent: &File, name: &str) -> Result<Option<File>, Error> {
    let s = match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(s) => s,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(Error::Io),
    };
    if FileType::from_raw_mode(s.st_mode) != FileType::RegularFile {
        return Err(Error::Unsupported);
    }
    let f = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| Error::Io)?;
    let m = f.metadata().map_err(|_| Error::Io)?;
    if !m.is_file() || m.dev() != s.st_dev || m.ino() != s.st_ino {
        return Err(Error::Conflict);
    }
    Ok(Some(f))
}
/// Create candidate/control bytes exclusively and synchronize the file and containing directory.
pub(crate) fn create(parent: &File, name: &str, bytes: &[u8]) -> Result<(), Error> {
    create_checked(parent, name, bytes, || Ok(()))
}
/// Stage bounded chunks while the actual owner observes stop/policy changes before each write.
pub(crate) fn create_checked(
    parent: &File,
    name: &str,
    bytes: &[u8],
    check: impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    check()?;
    let mut f = create_file(parent, name)?;
    for chunk in bytes.chunks(8192) {
        check()?;
        f.write_all(chunk).map_err(|_| Error::Io)?;
    }
    check()?;
    f.sync_all().map_err(|_| Error::Io)?;
    parent.sync_all().map_err(|_| Error::Io)
}
/// Create a no-clobber private regular file; never truncate an existing name.
pub(crate) fn create_file(parent: &File, name: &str) -> Result<File, Error> {
    openat(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|e| {
        if e == rustix::io::Errno::EXIST {
            Error::Conflict
        } else {
            Error::Io
        }
    })
}
/// Open a regular append/lock file without replacement or symlink traversal.
pub(crate) fn append_file(parent: &File, name: &str) -> Result<File, Error> {
    let f = openat(
        parent,
        name,
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::APPEND
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|_| Error::Io)?;
    let m = f.metadata().map_err(|_| Error::Io)?;
    if !m.is_file() || m.nlink() != 1 || m.uid() != rustix::process::geteuid().as_raw() {
        return Err(Error::Context);
    }
    parent.sync_all().map_err(|_| Error::Io)?;
    Ok(f)
}
/// Bound control input before constructing parser-owned storage; no unlimited read-to-end fallback.
pub(crate) fn control<T: serde::de::DeserializeOwned>(
    parent: &File,
    name: &str,
    max: u64,
) -> Result<T, Error> {
    control_checked(parent, name, max, || Ok(()))
}
/// Decode bounded control bytes with the live executor checked before each parser buffer refill.
pub(crate) fn control_checked<T: serde::de::DeserializeOwned>(
    parent: &File,
    name: &str,
    max: u64,
    check: impl Fn() -> Result<(), Error>,
) -> Result<T, Error> {
    check()?;
    let mut f = regular(parent, name)?.ok_or(Error::Recovery)?;
    let before = f.metadata().map_err(|_| Error::Io)?;
    if before.len() > max {
        return Err(Error::Budget);
    }
    let reader = CheckedReader {
        inner: (&mut f).take(max),
        check: &check,
    };
    let result = serde_json::from_reader(std::io::BufReader::with_capacity(8192, reader));
    check()?;
    let value = result.map_err(|_| Error::Recovery)?;
    if stamp(&before) != stamp(&f.metadata().map_err(|_| Error::Io)?) {
        return Err(Error::Recovery);
    }
    Ok(value)
}
/// Borrow the actual reader and executor stop check without allocating a second read buffer.
struct CheckedReader<R, F> {
    /// Already bounded native reader.
    inner: R,
    /// Exact live executor check.
    check: F,
}
impl<R: Read, F: Fn() -> Result<(), Error>> Read for CheckedReader<R, F> {
    /// Stop before each controllable native read; the owner returns the original typed check error.
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        (self.check)().map_err(|_| std::io::Error::other("execution stopped"))?;
        self.inner.read(bytes)
    }
}
/// Synchronize a control record without intermediate JSON trees or truncation of previous evidence.
pub(crate) fn control_new<T: serde::Serialize>(
    parent: &File,
    name: &str,
    value: &T,
) -> Result<(), Error> {
    control_new_checked(parent, name, value, || Ok(()))
}
/// Stop before each controlled serialization write; preserve partial evidence and the typed stop error.
pub(crate) fn control_new_checked<T: serde::Serialize>(
    parent: &File,
    name: &str,
    value: &T,
    check: impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    check()?;
    let mut f = create_file(parent, name)?;
    let result = serde_json::to_writer(
        CheckedWriter {
            inner: &mut f,
            check: &check,
        },
        value,
    );
    check()?;
    result.map_err(|_| Error::Io)?;
    let result = CheckedWriter {
        inner: &mut f,
        check: &check,
    }
    .write_all(b"\n");
    check()?;
    result.map_err(|_| Error::Io)?;
    f.sync_all().map_err(|_| Error::Io)?;
    check()?;
    parent.sync_all().map_err(|_| Error::Io)
}
/// Borrow one actual native writer with no intermediate output allocation.
struct CheckedWriter<W, F> {
    /// Actual exclusive evidence descriptor.
    inner: W,
    /// Exact current executor/policy check.
    check: F,
}
impl<W: Write, F: Fn() -> Result<(), Error>> Write for CheckedWriter<W, F> {
    /// Observe the owner before each controllable write, including partial-write retries.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.check)().map_err(|_| std::io::Error::other("execution stopped"))?;
        self.inner.write(bytes)
    }
    /// Observe the owner before flushing the actual writer.
    fn flush(&mut self) -> std::io::Result<()> {
        (self.check)().map_err(|_| std::io::Error::other("execution stopped"))?;
        self.inner.flush()
    }
}
/// Write one retained record with stop checks on every actual partial-write attempt.
/// Preserve the typed stop error and classify unsettled native I/O as Indeterminate.
pub(crate) fn write_checked(
    file: &mut File,
    bytes: &[u8],
    check: &impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    check()?;
    let result = CheckedWriter { inner: file, check }.write_all(bytes);
    check()?;
    result.map_err(|_| Error::Indeterminate)
}
/// Read exact bounded staged UTF-8 bytes, checking native version/EOF before they become Git input.
pub(crate) fn text(parent: &File, name: &str, max: u64) -> Result<String, Error> {
    text_checked(parent, name, max, || Ok(()))
}
/// Read only the pre-admitted aggregate body remainder with cancellation checks between chunks.
pub(crate) fn text_checked(
    parent: &File,
    name: &str,
    max: u64,
    check: impl Fn() -> Result<(), Error>,
) -> Result<String, Error> {
    check()?;
    let mut f = regular(parent, name)?.ok_or(Error::Recovery)?;
    let before = f.metadata().map_err(|_| Error::Io)?;
    if before.len() > max {
        return Err(Error::Budget);
    }
    let len = usize::try_from(before.len()).map_err(|_| Error::Budget)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(|_| Error::Budget)?;
    bytes.resize(len, 0);
    for chunk in bytes.chunks_mut(8192) {
        check()?;
        f.read_exact(chunk).map_err(|_| Error::Recovery)?;
    }
    let mut extra = [0];
    check()?;
    if f.read(&mut extra).map_err(|_| Error::Io)? != 0
        || stamp(&before) != stamp(&f.metadata().map_err(|_| Error::Io)?)
    {
        return Err(Error::Recovery);
    }
    String::from_utf8(bytes).map_err(|_| Error::Recovery)
}
/// Hash complete actual regular bytes through bounded scratch and before/after metadata checks.
pub(crate) fn version(f: File, max: u64, deadline: std::time::Instant) -> Result<Version, Error> {
    version_checked(f, max, deadline, || Ok(()))
}
/// Hash bounded chunks while the owning executor observes cancellation before every native read.
pub(crate) fn version_checked(
    mut f: File,
    max: u64,
    deadline: std::time::Instant,
    check: impl Fn() -> Result<(), Error>,
) -> Result<Version, Error> {
    check()?;
    let before = f.metadata().map_err(|_| Error::Io)?;
    if !before.is_file() {
        return Err(Error::Unsupported);
    }
    if before.len() > max {
        return Err(Error::Budget);
    }
    let mut h = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut bytes = 0u64;
    loop {
        check()?;
        if std::time::Instant::now() >= deadline {
            return Err(Error::Budget);
        }
        let n = f.read(&mut buf).map_err(|_| Error::Io)?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or(Error::Budget)?;
        if bytes > max {
            return Err(Error::Budget);
        }
        h.update(&buf[..n]);
    }
    let after = f.metadata().map_err(|_| Error::Io)?;
    if stamp(&before) != stamp(&after) || bytes != before.len() {
        return Err(Error::Conflict);
    }
    Ok(Version {
        device: before.dev(),
        inode: before.ino(),
        length: bytes,
        modified: (before.mtime(), before.mtime_nsec()),
        changed: (before.ctime(), before.ctime_nsec()),
        digest: format!("{:x}", h.finalize()),
    })
}
/// Compare complete stat identities without treating modification time as content or move identity.
fn stamp(m: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
/// Observe selected current bytes; absent final names/ancestors are represented explicitly.
pub(crate) fn observe(
    root: &File,
    path: &str,
    max: u64,
    deadline: std::time::Instant,
) -> Result<Option<Version>, Error> {
    let (p, n) = match parent(root, path, false) {
        Ok(v) => v,
        Err(Error::Missing) => return Ok(None),
        Err(e) => return Err(e),
    };
    regular(&p, &n)?
        .map(|f| version(f, max, deadline))
        .transpose()
}
/// Retain the selected old entry by atomic no-replace rename on the same filesystem.
pub(crate) fn displace(
    parent: &File,
    name: &str,
    recovery: &File,
    target: &str,
) -> Result<(), Error> {
    rustix::fs::renameat_with(parent, name, recovery, target, RenameFlags::NOREPLACE).map_err(
        |e| {
            if e == rustix::io::Errno::EXIST {
                Error::Conflict
            } else {
                Error::Io
            }
        },
    )?;
    parent.sync_all().map_err(|_| Error::Indeterminate)?;
    recovery.sync_all().map_err(|_| Error::Indeterminate)
}
/// Copy only this operation's candidate to an independent inode, then install without replacement.
/// The caller pre-admits both copies and the bounded buffer; failed copies stay visible for recovery.
/// Immutable new-N evidence is never linked to editable home bytes.
/// Expected describes the already verified stage; native drift or digest mismatch returns Recovery before publication.
pub(crate) fn install_evidence(
    stage: &File,
    name: &str,
    parent: &File,
    target: &str,
    expected: &Version,
    max: u64,
    check: impl Fn() -> Result<(), Error>,
) -> Result<(), Error> {
    check()?;
    let mut source = regular(stage, name)?.ok_or(Error::Recovery)?;
    let before = source.metadata().map_err(|_| Error::Io)?;
    if stamp(&before)
        != (
            expected.device,
            expected.inode,
            expected.length,
            expected.modified.0,
            expected.modified.1,
            expected.changed.0,
            expected.changed.1,
        )
    {
        return Err(Error::Recovery);
    }
    if before.len().checked_mul(2).is_none_or(|n| n > max) {
        return Err(Error::Budget);
    }
    let copy_name = format!("copy-{name}");
    let mut copy = create_file(stage, &copy_name)?;
    let mut remaining = before.len();
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    while remaining > 0 {
        check()?;
        let limit = remaining.min(buffer.len() as u64) as usize;
        let n = source.read(&mut buffer[..limit]).map_err(|_| Error::Io)?;
        if n == 0 {
            return Err(Error::Recovery);
        }
        check()?;
        copy.write_all(&buffer[..n]).map_err(|_| Error::Io)?;
        digest.update(&buffer[..n]);
        remaining -= n as u64;
    }
    let mut extra = [0];
    check()?;
    if source.read(&mut extra).map_err(|_| Error::Io)? != 0
        || format!("{:x}", digest.finalize()) != expected.digest
        || stamp(&before) != stamp(&source.metadata().map_err(|_| Error::Io)?)
    {
        return Err(Error::Recovery);
    }
    copy.sync_all().map_err(|_| Error::Io)?;
    stage.sync_all().map_err(|_| Error::Io)?;
    drop(copy);
    check()?;
    displace(stage, &copy_name, parent, target)
}
/// Install a staged inode atomically without replacing any competing destination.
/// Bootstrap payloads have no historical text-save receipt; text operations use install_evidence.
pub(crate) fn install(stage: &File, name: &str, parent: &File, target: &str) -> Result<(), Error> {
    rustix::fs::linkat(stage, name, parent, target, AtFlags::empty()).map_err(|e| {
        if e == rustix::io::Errno::EXIST {
            Error::Conflict
        } else {
            Error::Indeterminate
        }
    })?;
    parent.sync_all().map_err(|_| Error::Indeterminate)
}
