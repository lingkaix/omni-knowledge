//! Held-directory operations. Every component is opened with NOFOLLOW.
use crate::Error;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, mkdirat, openat, statat};
use std::fs::File;
use std::path::{Component, Path};

/// Bind an absolute directory without following any symlink ancestor.
pub(crate) fn bind(path: &Path) -> Result<File, Error> {
    if !path.is_absolute() {
        return Err(Error::Hostile);
    }
    let root = File::open("/").map_err(|_| Error::Io)?;
    directory(
        &root,
        path.strip_prefix("/").map_err(|_| Error::Hostile)?,
        false,
    )
}

/// Walk ordinary relative components, optionally creating absent directories.
pub(crate) fn directory(root: &File, path: &Path, create: bool) -> Result<File, Error> {
    let mut dir = root.try_clone().map_err(|_| Error::Io)?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(Error::Hostile);
        };
        if create {
            match mkdirat(&dir, name, Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(classify(error)),
            }
        }
        dir = openat(
            &dir,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(classify)?;
    }
    Ok(dir)
}

/// Open a regular file through held parents. Exclusive creation never truncates evidence.
pub(crate) fn file(root: &File, path: &Path, create: bool) -> Result<File, Error> {
    let parent = directory(root, path.parent().ok_or(Error::Hostile)?, create)?;
    let name = path.file_name().ok_or(Error::Hostile)?;
    if !create {
        let stat = statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(classify)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(Error::Hostile);
        }
    }
    let flags = if create {
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL
    } else {
        OFlags::RDONLY
    };
    let fd = openat(
        &parent,
        name,
        flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(classify)?;
    if !fd.metadata().map_err(|_| Error::Io)?.is_file() {
        return Err(Error::Hostile);
    }
    Ok(fd)
}

/// Observe a marker without following it. Unknown special nodes are refused.
pub(crate) fn exists(root: &File, name: &str) -> Result<bool, Error> {
    match statat(root, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => Ok(true),
        Ok(_) => Err(Error::Hostile),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(error) => Err(classify(error)),
    }
}

/// Link an already confined name into a held destination parent, without replacement.
pub(crate) fn link(root: &File, name: &str, destination: &Path) -> Result<bool, Error> {
    let parent = bind(destination.parent().ok_or(Error::Hostile)?)?;
    match rustix::fs::linkat(
        root,
        name,
        &parent,
        destination.file_name().ok_or(Error::Hostile)?,
        AtFlags::empty(),
    ) {
        Ok(()) => Ok(true),
        Err(rustix::io::Errno::EXIST) => Ok(false),
        Err(error) => Err(classify(error)),
    }
}

/// Map native errors without exposing paths.
fn classify(error: rustix::io::Errno) -> Error {
    match error {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => Error::Hostile,
        rustix::io::Errno::EXIST => Error::Conflict,
        _ => Error::Io,
    }
}
