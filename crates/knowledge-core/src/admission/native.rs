//! Fixed native memory and selected-descriptor volume facts; no process scans or shell parsers.
use super::{Error, Volume};
use std::fs::File;

/// Observe physical RAM in bytes, not free-page availability or a guessed cgroup/process ceiling.
#[cfg(target_os = "linux")]
pub(super) fn physical_memory() -> Result<u64, Error> {
    let info = rustix::system::sysinfo();
    let bytes = u128::from(info.totalram) * u128::from(info.mem_unit);
    let bytes = u64::try_from(bytes).map_err(|_| Error::Arithmetic)?;
    if bytes == 0 {
        return Err(Error::NativeFacts);
    }
    Ok(bytes)
}

/// Observe macOS physical RAM through the already-locked safe native binding; zero is not unlimited.
#[cfg(target_os = "macos")]
pub(super) fn physical_memory() -> Result<u64, Error> {
    let mut system = sysinfo::System::new();
    system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
    let bytes = system.total_memory();
    if bytes == 0 {
        return Err(Error::NativeFacts);
    }
    Ok(bytes)
}

/// Refuse unsupported platform observations instead of inventing a startup budget.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) fn physical_memory() -> Result<u64, Error> {
    Err(Error::Unsupported)
}

/// Validate an already selected private directory and sample its held volume in bytes.
/// The trusted cache owner still supplies locality, placement, ACL and current source admission.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn volume(directory: &File) -> Result<Volume, Error> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let m = directory.metadata().map_err(|_| Error::NativeFacts)?;
    if !m.is_dir()
        || m.nlink() == 0
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.permissions().mode() & 0o7777 != 0o700
    {
        return Err(Error::UnsafeDirectory);
    }
    let stat = rustix::fs::fstatvfs(directory).map_err(|_| Error::NativeFacts)?;
    let unit = u128::from(stat.f_frsize);
    if unit == 0 {
        return Err(Error::NativeFacts);
    }
    let capacity_bytes =
        u64::try_from(u128::from(stat.f_blocks) * unit).map_err(|_| Error::Arithmetic)?;
    let available_bytes =
        u64::try_from(u128::from(stat.f_bavail) * unit).map_err(|_| Error::Arithmetic)?;
    if capacity_bytes == 0 || available_bytes > capacity_bytes {
        return Err(Error::NativeFacts);
    }
    let proportional = capacity_bytes / 20 + u64::from(capacity_bytes % 20 != 0);
    Ok(Volume {
        device: m.dev(),
        directory: m.ino(),
        capacity_bytes,
        available_bytes,
        floor_bytes: (2 * 1024 * 1024 * 1024).max(proportional),
    })
}

/// Keep the ordinary Core portable while unsupported native admission fails closed.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) fn volume(_directory: &File) -> Result<Volume, Error> {
    Err(Error::Unsupported)
}
