//! Small Unix descriptor operations; no pathname fallback or recursive deletion.
use super::{CacheError, DirectoryIdentity};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, openat, statat};
use std::fs::File;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};

/// File-version evidence attached to an opened object, not a content digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    /// Filesystem object identity.
    pub identity: DirectoryIdentity,
    /// Logical byte count.
    pub length: u64,
    /// Change time, including nanoseconds.
    pub changed: (i64, i64),
    /// Modification time, including nanoseconds.
    pub modified: (i64, i64),
    /// Link count, also checked around reads.
    pub links: u64,
}

/// Read descriptor metadata without resolving a pathname.
pub(super) fn stamp(file: &File) -> Result<Stamp, CacheError> {
    let m = file.metadata().map_err(|_| CacheError::Io)?;
    Ok(Stamp {
        identity: DirectoryIdentity {
            device: m.dev(),
            inode: m.ino(),
        },
        length: m.len(),
        changed: (m.ctime(), m.ctime_nsec()),
        modified: (m.mtime(), m.mtime_nsec()),
        links: m.nlink(),
    })
}

/// Require current-user ownership, exact permission bits, and the expected native file kind.
pub(super) fn private(file: &File, directory: bool, mode: u32) -> Result<(), CacheError> {
    let m = file.metadata().map_err(|_| CacheError::Io)?;
    if m.uid() != rustix::process::geteuid().as_raw()
        || m.permissions().mode() & 0o7777 != mode
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        return Err(CacheError::UnsafeRoot);
    }
    Ok(())
}

/// Open a directory component without following a final symlink; return only directory descriptors.
pub(super) fn directory(parent: &File, name: &str) -> Result<File, CacheError> {
    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        if error == rustix::io::Errno::NOENT {
            CacheError::Missing
        } else {
            CacheError::UnsafeRoot
        }
    })
}

/// Walk only normalized absolute components, refusing symlinks even in ancestors.
/// Return every physical ancestor identity so bind-mounted aliases cannot pass lexical overlap alone.
pub(super) fn walk(path: &Path) -> Result<(File, Vec<DirectoryIdentity>), CacheError> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(CacheError::UnsafeRoot);
    }
    let mut file = File::open("/").map_err(|_| CacheError::Io)?;
    let mut chain = vec![stamp(&file)?.identity];
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = name.to_str().ok_or(CacheError::UnsafeRoot)?;
                file = directory(&file, name)?;
                chain.push(stamp(&file)?.identity);
            }
            _ => return Err(CacheError::UnsafeRoot),
        }
    }
    Ok((file, chain))
}

/// Check a directory binding and its permissions without trusting its name alone.
pub(super) fn bound_directory(parent: &File, name: &str, held: &File) -> Result<(), CacheError> {
    let current = directory(parent, name)?;
    private(&current, true, 0o700)?;
    private(held, true, 0o700)?;
    if stamp(&current)?.identity != stamp(held)?.identity {
        return Err(CacheError::Changed);
    }
    Ok(())
}

/// Create one private directory only after admission; reject existing substituted nodes.
pub(super) fn child(parent: &File, name: &str) -> Result<File, CacheError> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(_) => return Err(CacheError::Io),
    }
    let file = directory(parent, name)?;
    private(&file, true, 0o700)?;
    Ok(file)
}

/// Open an existing regular file only; no symlink following or blocking FIFO reads.
/// Check kind before open and again before any byte I/O; hostile same-UID mknod races are not isolation.
pub(super) fn regular(parent: &File, name: &str, mode: u32) -> Result<File, CacheError> {
    let info = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| {
        if e == rustix::io::Errno::NOENT {
            CacheError::Missing
        } else {
            CacheError::Io
        }
    })?;
    if FileType::from_raw_mode(info.st_mode) != FileType::RegularFile {
        return Err(CacheError::UnsafeRoot);
    }
    let file = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| CacheError::UnsafeRoot)?;
    private(&file, false, mode)?;
    if stamp(&file)?.identity
        != (DirectoryIdentity {
            device: info.st_dev as u64,
            inode: info.st_ino as u64,
        })
    {
        return Err(CacheError::Changed);
    }
    Ok(file)
}

/// Exclusively create a candidate or owner file; never open an existing node for writing.
pub(super) fn create(parent: &File, name: &str) -> Result<File, CacheError> {
    let file = openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|e| {
        if e == rustix::io::Errno::EXIST {
            CacheError::Conflict
        } else {
            CacheError::Io
        }
    })?;
    private(&file, false, 0o600)?;
    Ok(file)
}

/// Open a known private regular owner inode for locking, with before/after identity checks.
pub(super) fn owner(parent: &File) -> Result<File, CacheError> {
    match create(parent, ".owner") {
        Ok(file) => Ok(file),
        Err(CacheError::Conflict) => {
            let old = regular(parent, ".owner", 0o600)?;
            let file = openat(
                parent,
                ".owner",
                OFlags::RDWR
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::NOCTTY
                    | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map(File::from)
            .map_err(|_| CacheError::UnsafeRoot)?;
            private(&file, false, 0o600)?;
            if stamp(&old)?.identity != stamp(&file)?.identity {
                return Err(CacheError::Changed);
            }
            Ok(file)
        }
        Err(error) => Err(error),
    }
}

/// Scan a bounded number of native entries, charging all file/link bytes without following targets.
/// Unknown directories require inspection rather than hiding an unbounded subtree behind a warning.
pub(super) fn usage(
    root: &File,
    max_entries: usize,
    max_disk: u64,
) -> Result<super::Usage, CacheError> {
    /// Descend only the generated namespace/fanout layout, retaining every other node.
    fn scan(
        dir: &File,
        depth: u8,
        out: &mut super::Usage,
        maximum: usize,
        disk: u64,
    ) -> Result<(), CacheError> {
        // A fresh open description prevents shared readdir offsets from bypassing later scans.
        let fresh = directory(dir, ".")?;
        let entries = Dir::read_from(&fresh).map_err(|_| CacheError::Io)?;
        for entry in entries {
            let entry = entry.map_err(|_| CacheError::Io)?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            out.entries = out.entries.checked_add(1).ok_or(CacheError::Budget)?;
            if out.entries > maximum {
                return Err(CacheError::Budget);
            }
            let info =
                statat(dir, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| CacheError::Changed)?;
            let kind = FileType::from_raw_mode(info.st_mode);
            let text = name.to_str().ok();
            let hex = |n: usize| {
                text.is_some_and(|s| {
                    s.len() == n
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            };
            if kind == FileType::Directory {
                if !((depth == 0 && hex(64)) || (depth == 1 && hex(2))) {
                    return Err(CacheError::InspectionRequired);
                }
                let next = directory(dir, text.ok_or(CacheError::InspectionRequired)?)?;
                private(&next, true, 0o700)?;
                scan(&next, depth + 1, out, maximum, disk)?;
            } else {
                let size =
                    u64::try_from(info.st_size).map_err(|_| CacheError::InspectionRequired)?;
                out.charged_bytes = out
                    .charged_bytes
                    .checked_add(size)
                    .ok_or(CacheError::Budget)?;
                if out.charged_bytes > disk {
                    return Err(CacheError::Budget);
                }
                if depth == 2
                    && text.is_some_and(|s| s.starts_with(".candidate-"))
                    && kind == FileType::RegularFile
                {
                    out.retained_candidates += 1;
                } else if !((depth == 0 && text == Some(".owner") && kind == FileType::RegularFile)
                    || (depth == 2 && hex(64) && kind == FileType::RegularFile))
                {
                    out.unknown_entries += 1;
                }
            }
        }
        Ok(())
    }
    let mut out = super::Usage {
        charged_bytes: 0,
        entries: 0,
        unknown_entries: 0,
        retained_candidates: 0,
    };
    scan(root, 0, &mut out, max_entries, max_disk)?;
    Ok(out)
}
