//! Native-owner FD handoff after exact private copying; owner results and unsettled state never disappear.
use crate::{
    ContentDigest, Error, VerifiedSource, Work, gate, native, note_file, operation_id, source,
};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::{fs::File, path::Path};

/// Native-owner result plus the independent post-callback source observation.
/// An owner may already have effects or unsettled children; a late package error must not drop that state.
pub struct OwnerHandoff<T> {
    /// Actual callback result, including its original native execution/recovery/unsettled errors.
    pub owner: T,
    /// Fresh control and held-copy version check after callback completion, not an owner success claim.
    pub source_revalidation: Result<(), Error>,
}

impl VerifiedSource {
    /// Copy this exact selected stream into caller-selected private evidence, then lend a read-only File.
    /// `private` is a previously admitted absolute private parent; `operation` must be a fresh confined ID.
    /// The callback receives only the copied selected payload, never the original archive descriptor.
    /// The handed descriptor is opened `O_RDONLY` and `O_NOFOLLOW` on the copied name while the writer remains open.
    /// It must be that same inode, length, and timestamp, and its access mode must be read-only, before it is rehashed.
    /// The writer stays open until the owner callback returns. The private path is not a grant.
    /// On Linux, `/proc/self/fd` of the writer is only an extra identity check. It is not the handed capability.
    /// The same open is used on macOS. This function does not report `Unsupported` when `/proc` is absent.
    /// It uses the same Work; native History additionally consumes the host's actual existing Claim.
    /// This supplies the concrete held-file input required by History 3ba76ed without running a second Git engine.
    /// Pre-callback errors return Err and preserve partial private evidence; existing operations refuse replay.
    /// After callback entry the actual owner result is always retained, even if cancellation/drift follows.
    /// The caller must inspect both fields, retain native unsettled claims, and settle private files before
    /// finishing its holding. This method never finishes the caller's holding or removes unknown evidence.
    pub fn with_private_file<T>(
        &mut self,
        private: &Path,
        operation: &str,
        work: &mut Work<'_>,
        body: impl FnOnce(&File, &ContentDigest, &mut Work<'_>) -> T,
    ) -> Result<OwnerHandoff<T>, Error> {
        operation_id(operation)?;
        self.revalidate(work)?;
        let root = native::bind(private, work)?;
        note_file(work)?;
        rustix::fs::mkdirat(&root, operation, rustix::fs::Mode::from_raw_mode(0o700)).map_err(
            |e| {
                if e == rustix::io::Errno::EXIST {
                    Error::Interrupted
                } else {
                    Error::Io
                }
            },
        )?;
        let stage = native::directory(&root, Path::new(operation), false, work)?;
        let mut output = native::file(&stage, Path::new("source"), true, work)?;
        self.copy_to(&mut output, work)?;
        output.sync_all().map_err(|_| Error::Io)?;
        let file = read_only_same_inode(&output, &stage, work)?;
        let digest = self.digest();
        let verified = source::range(&file, 0, self.len(), digest, work)?;
        note_file(work)?;
        if file.metadata().map_err(|_| Error::Io)?.len() != self.len() {
            return Err(Error::Checksum);
        }
        self.revalidate(work)?;
        gate(work)?;
        let owner = body(&file, digest, work);
        let source_revalidation = verified
            .revalidate(work)
            .and_then(|()| self.revalidate(work));
        drop(output);
        Ok(OwnerHandoff {
            owner,
            source_revalidation,
        })
    }
}

/// Open `source` read-only while `file` still holds the copied inode, then prove that opened descriptor.
/// Returns `Error::Io` when the descriptor cannot be opened and `Error::Drift` when identity or access mode differs.
fn read_only_same_inode(file: &File, dir: &File, work: &mut Work<'_>) -> Result<File, Error> {
    note_file(work)?;
    let before = file.metadata().map_err(|_| Error::Io)?;
    let opened = rustix::fs::openat(
        dir,
        "source",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| Error::Io)?;
    let after = opened.metadata().map_err(|_| Error::Io)?;
    if !same_copied_file(&before, &after) {
        return Err(Error::Drift);
    }
    let flags = rustix::fs::fcntl_getfl(&opened).map_err(|_| Error::Io)?;
    if flags & rustix::fs::OFlags::RWMODE != rustix::fs::OFlags::RDONLY {
        return Err(Error::Drift);
    }
    if cfg!(target_os = "linux") {
        confirm_linux_proc(file, &before)?;
    }
    Ok(opened)
}

/// Compare the file that received the copy with the descriptor that will be handed to the owner.
fn same_copied_file(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    after.is_file()
        && after.dev() == before.dev()
        && after.ino() == before.ino()
        && after.len() == before.len()
        && after.mtime() == before.mtime()
        && after.mtime_nsec() == before.mtime_nsec()
        && after.ctime() == before.ctime()
        && after.ctime_nsec() == before.ctime_nsec()
}

/// When `/proc` is present, require the writer's descriptor to name the same copied inode.
/// A missing `/proc` entry does not choose a different open and does not fail the handoff.
fn confirm_linux_proc(file: &File, before: &std::fs::Metadata) -> Result<(), Error> {
    let link = format!("/proc/self/fd/{}", file.as_raw_fd());
    let Ok(proc_file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::CLOEXEC.bits() as i32)
        .open(link)
    else {
        return Ok(());
    };
    let proc_meta = proc_file.metadata().map_err(|_| Error::Io)?;
    if same_copied_file(before, &proc_meta) {
        Ok(())
    } else {
        Err(Error::Drift)
    }
}
