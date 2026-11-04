//! Descriptor-relative Unix helpers for one bound directory.
//! Paths are not canonicalized, and symlink components are not followed.
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, Stat, fstat, openat, statat};
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::{Component, Path};

/// Failures from descriptor operations. `Changed` is an observed identity race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeError {
    /// Operating-system error, classified by the caller.
    Os(rustix::io::Errno),
    /// The node changed between the check and the open or read.
    Changed,
    /// The node is a symlink, special file, or otherwise not an ordinary object.
    Unsupported,
}

/// Directory bound by a no-follow walk from `/`.
pub(crate) struct Bound {
    /// Directory descriptor for the final component.
    pub file: File,
    /// `st_dev` of that descriptor.
    pub device: u64,
    /// `st_ino` of that descriptor.
    pub inode: u64,
}

/// One no-follow metadata sample. Times that do not fit the signed-nanosecond field stay absent.
#[derive(Clone, Debug)]
pub(crate) struct Observed {
    /// Ordinary directory, ordinary file, or rejected node.
    pub kind: NodeKind,
    /// Logical byte length for a representable node; zero when the kind is unsupported.
    pub size: u64,
    /// Modification time in signed nanoseconds, when the kernel value fits.
    pub modified_ns: Option<i128>,
    /// Status-change time in signed nanoseconds, when the kernel value fits.
    pub changed_ns: Option<i128>,
    /// Device number from the no-follow stat.
    pub device: u64,
    /// Inode number from the no-follow stat. Not a stable ID by itself.
    pub inode: u64,
    /// Birth time when this platform and filesystem actually reported one.
    pub birth_ns: Option<i128>,
    /// Generation when this platform actually reported a nonzero generation.
    pub generation: Option<String>,
}

/// Confined object class used by the adapter. Symlinks and specials are unsupported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Ordinary directory reached without following a symlink.
    Directory,
    /// Regular file.
    File,
    /// Symlink, fifo, device, socket, or unknown node.
    Unsupported,
}

/// Bind `path` without resolving `..`, `.`, or symlinks.
pub(crate) fn bind(path: &Path) -> Result<Bound, NativeError> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(NativeError::Os(rustix::io::Errno::INVAL));
    }
    let mut file = File::open("/").map_err(io_error)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = name
                    .to_str()
                    .ok_or(NativeError::Os(rustix::io::Errno::INVAL))?;
                file = open_directory(&file, name)?;
            }
            _ => return Err(NativeError::Os(rustix::io::Errno::INVAL)),
        }
    }
    let (device, inode, kind) = identity_of(&file)?;
    if kind != NodeKind::Directory {
        return Err(NativeError::Unsupported);
    }
    Ok(Bound {
        file,
        device,
        inode,
    })
}

/// Device, inode, and kind of an already opened descriptor.
pub(crate) fn identity_of(file: &File) -> Result<(u64, u64, NodeKind), NativeError> {
    let stat = fstat(file).map_err(NativeError::Os)?;
    let device = as_u64(stat.st_dev).ok_or(NativeError::Os(rustix::io::Errno::IO))?;
    let inode = as_u64(stat.st_ino).ok_or(NativeError::Os(rustix::io::Errno::IO))?;
    Ok((device, inode, kind_of(&stat)))
}

/// Whether `file` still has the device, inode, kind, size, and timestamps in `observed`.
/// Birth and generation are not on this `fstat` sample.
pub(crate) fn same_sample(file: &File, observed: &Observed) -> Result<bool, NativeError> {
    let stat = fstat(file).map_err(NativeError::Os)?;
    let Some(device) = as_u64(stat.st_dev) else {
        return Ok(false);
    };
    let Some(inode) = as_u64(stat.st_ino) else {
        return Ok(false);
    };
    let size = as_i64(stat.st_size).and_then(|value| u64::try_from(value).ok());
    Ok(kind_of(&stat) == observed.kind
        && device == observed.device
        && inode == observed.inode
        && size == Some(observed.size)
        && stamp(stat.st_mtime, stat.st_mtime_nsec) == observed.modified_ns
        && stamp(stat.st_ctime, stat.st_ctime_nsec) == observed.changed_ns)
}

/// No-follow metadata for one directory entry.
pub(crate) fn observe(dir: &File, name: &std::ffi::CStr) -> Result<Observed, NativeError> {
    let stat = statat(dir, name, AtFlags::SYMLINK_NOFOLLOW).map_err(NativeError::Os)?;
    finish_observe(dir, name, &stat)
}

/// Open a directory component without following a symlink.
pub(crate) fn open_directory(parent: &File, name: &str) -> Result<File, NativeError> {
    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(NativeError::Os)
}

/// Open one regular file. A symlink or special node is unsupported and is not read.
/// The opened inode must match the no-follow stat taken immediately before the open.
pub(crate) fn open_regular(parent: &File, name: &std::ffi::CStr) -> Result<File, NativeError> {
    let before = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(NativeError::Os)?;
    if kind_of(&before) != NodeKind::File {
        return Err(NativeError::Unsupported);
    }
    let file = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(NativeError::Os)?;
    let after = fstat(&file).map_err(NativeError::Os)?;
    if kind_of(&after) != NodeKind::File
        || after.st_dev != before.st_dev
        || after.st_ino != before.st_ino
    {
        return Err(NativeError::Changed);
    }
    Ok(file)
}

/// Read one directory stream from an independent descriptor, so later scans do not share an offset.
pub(crate) fn read_dir(dir: &File) -> Result<Dir, NativeError> {
    Dir::read_from(dir).map_err(NativeError::Os)
}

/// Read at `offset`, retrying a short run of interruptions. This is not a deadline.
pub(crate) fn read_at(file: &File, buf: &mut [u8], offset: u64) -> Result<usize, NativeError> {
    let mut interrupted = 0u8;
    loop {
        match file.read_at(buf, offset) {
            Ok(n) => return Ok(n),
            Err(error) if error.kind() == io::ErrorKind::Interrupted && interrupted < 8 => {
                interrupted += 1;
            }
            Err(error) => return Err(io_error(error)),
        }
    }
}

fn finish_observe(dir: &File, name: &std::ffi::CStr, stat: &Stat) -> Result<Observed, NativeError> {
    let device = as_u64(stat.st_dev).ok_or(NativeError::Os(rustix::io::Errno::IO))?;
    let inode = as_u64(stat.st_ino).ok_or(NativeError::Os(rustix::io::Errno::IO))?;
    let kind = kind_of(stat);
    let size = match as_i64(stat.st_size).and_then(|value| u64::try_from(value).ok()) {
        Some(size) => size,
        None => {
            return Ok(unsupported(device, inode));
        }
    };
    let (birth_ns, generation) = birth_and_generation(dir, name, stat);
    Ok(Observed {
        kind,
        size,
        modified_ns: stamp(stat.st_mtime, stat.st_mtime_nsec),
        changed_ns: stamp(stat.st_ctime, stat.st_ctime_nsec),
        device,
        inode,
        birth_ns,
        generation,
    })
}

fn unsupported(device: u64, inode: u64) -> Observed {
    Observed {
        kind: NodeKind::Unsupported,
        size: 0,
        modified_ns: None,
        changed_ns: None,
        device,
        inode,
        birth_ns: None,
        generation: None,
    }
}

fn kind_of(stat: &Stat) -> NodeKind {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::Directory => NodeKind::Directory,
        FileType::RegularFile => NodeKind::File,
        _ => NodeKind::Unsupported,
    }
}

fn stamp(sec: impl TryInto<i64> + Copy, nsec: impl TryInto<u64> + Copy) -> Option<i128> {
    let sec = sec.try_into().ok()?;
    let nsec = nsec.try_into().ok()?;
    if nsec >= 1_000_000_000 {
        return None;
    }
    i128::from(sec)
        .checked_mul(1_000_000_000)?
        .checked_add(i128::from(nsec))
}

#[cfg(target_os = "linux")]
fn birth_and_generation(
    dir: &File,
    name: &std::ffi::CStr,
    _stat: &Stat,
) -> (Option<i128>, Option<String>) {
    let flags = AtFlags::SYMLINK_NOFOLLOW | AtFlags::NO_AUTOMOUNT;
    let Ok(sample) = rustix::fs::statx(dir, name, flags, rustix::fs::StatxFlags::BTIME) else {
        return (None, None);
    };
    if sample.stx_mask & rustix::fs::StatxFlags::BTIME.bits() == 0 {
        return (None, None);
    }
    let birth = stamp(sample.stx_btime.tv_sec, u64::from(sample.stx_btime.tv_nsec));
    (birth, None)
}

#[cfg(target_os = "macos")]
fn birth_and_generation(
    _dir: &File,
    _name: &std::ffi::CStr,
    stat: &Stat,
) -> (Option<i128>, Option<String>) {
    let birth = if stat.st_birthtime == 0 && stat.st_birthtime_nsec == 0 {
        None
    } else {
        stamp(stat.st_birthtime, stat.st_birthtime_nsec)
    };
    let generation = if stat.st_gen == 0 {
        None
    } else {
        Some(stat.st_gen.to_string())
    };
    (birth, generation)
}

fn as_u64(value: impl TryInto<u64>) -> Option<u64> {
    value.try_into().ok()
}

fn as_i64(value: impl TryInto<i64>) -> Option<i64> {
    value.try_into().ok()
}

fn io_error(error: io::Error) -> NativeError {
    match error.raw_os_error() {
        Some(code) => NativeError::Os(rustix::io::Errno::from_raw_os_error(code)),
        None => NativeError::Os(rustix::io::Errno::IO),
    }
}
