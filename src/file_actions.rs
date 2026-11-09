//! Native actions for explicitly selected regular sources.
//! Callers confirm Trash before dispatch; source metadata and published indexes remain unchanged.

use crate::{AssetMetadata, CONFIG_FILE, KnowledgeBase};
use anyhow::{Context, Result, bail, ensure};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A completed native Trash operation; a destination is included only when macOS reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashReceipt {
    /// Original absolute source location.
    pub source: PathBuf,
    /// Actual system Trash destination, when returned by the native API.
    pub trashed_path: Option<PathBuf>,
}

/// One immediate live child, independent of the published retrieval snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    /// Canonical root-relative UTF-8 identity.
    pub relative: String,
    /// True for a real directory, including an empty directory.
    pub is_directory: bool,
    /// True when a directory is a marked child-library boundary; its children are not browsed here.
    pub is_nested_root: bool,
}

/// A completed file transfer; authored metadata uses the new relative identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferReceipt {
    /// Absolute original path; copies retain this source.
    pub source: PathBuf,
    /// Absolute destination successfully created without replacement.
    pub destination: PathBuf,
    /// Destination path relative to the selected knowledge root.
    pub relative: String,
}

/// Open exactly the selected knowledge root without redirecting to an ancestor library.
/// Canonicalizes a directory, validates its marker, and rejects a removed or relocated root before effects.
/// Explicit desktop jobs use this after root discovery; opening an arbitrary user-selected path can still discover an ancestor.
pub(crate) fn open_exact_root(root: &Path) -> Result<KnowledgeBase> {
    let selected = root
        .canonicalize()
        .context("Selected knowledge root is unavailable")?;
    ensure!(
        selected.is_dir(),
        "Selected knowledge root is not a directory"
    );
    let kb =
        KnowledgeBase::open(&selected).context("Selected knowledge root could not be opened")?;
    ensure!(
        kb.root == selected,
        "Selected knowledge root marker is missing or changed; reopen the knowledge library"
    );
    Ok(kb)
}

/// List immediate live children, including empty folders, without an index or a result cap.
/// `relative_folder` is empty for the root; internal state, symlinks, and root markers are omitted.
/// Returns an error when a folder is unreadable, unowned, or inside a nested knowledge root.
pub fn directory_children(root: &Path, relative_folder: &str) -> Result<Vec<DirectoryEntry>> {
    let kb = open_exact_root(root)?;
    let directory = owned_folder(&kb, relative_folder)?;
    let mut children = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".omni" || name == ".git" || name == CONFIG_FILE {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
            continue;
        }
        let relative = root_relative(&kb, &entry.path())?;
        let is_nested_root = if kind.is_dir() {
            crate::storage::has_marker(&entry.path())?
        } else {
            false
        };
        children.push(DirectoryEntry {
            relative,
            is_directory: kind.is_dir(),
            is_nested_root,
        });
    }
    children.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then(a.relative.cmp(&b.relative))
    });
    Ok(children)
}

/// Copy an explicitly selected regular file into an existing owned folder without overwrites.
/// Copies bytes and basic permissions; extended attributes and Finder labels are not copied.
/// An owned source contributes editable tags and description; an external source receives empty metadata.
/// Keeps source metadata and the published index unchanged; collisions and partial failures are explicit.
pub fn copy_into(
    root: &Path,
    external_source: &Path,
    destination_folder: &str,
) -> Result<TransferReceipt> {
    let kb = open_exact_root(root)?;
    let _writer = kb.writer_lock()?;
    let (source, source_metadata) = copy_source(&kb, external_source)?;
    let receipt = transfer_destination(&kb, &source, destination_folder)?;
    let mut input = open_regular_source(&source)?;
    let mut data = tempfile::NamedTempFile::new_in(receipt.destination.parent().unwrap())?;
    std::io::copy(&mut input, &mut data)?;
    data.as_file()
        .set_permissions(input.metadata()?.permissions())?;
    data.as_file().sync_all()?;
    let metadata = destination_metadata(&receipt.relative, source_metadata);
    commit_transfer(&kb, &metadata, || {
        data.persist_noclobber(&receipt.destination)
            .map_err(|error| error.error)
            .context("Destination already exists or copied file cannot be published")?;
        Ok(())
    })?;
    Ok(receipt)
}

/// Atomically move an owned file within this root without replacing a destination.
/// Creates the new editable metadata identity and retains the old sidecar for recovery.
/// Cross-filesystem moves fail without copying or deleting the source; use Copy instead.
pub fn move_within(
    root: &Path,
    source_relative: &str,
    destination_folder: &str,
) -> Result<TransferReceipt> {
    let kb = open_exact_root(root)?;
    let _writer = kb.writer_lock()?;
    let source = owned_source(&kb, source_relative)?;
    let receipt = transfer_destination(&kb, &source, destination_folder)?;
    let source_identity = root_relative(&kb, &source)?;
    let metadata =
        destination_metadata(&receipt.relative, Some(kb.read_metadata(&source_identity)?));
    commit_transfer(&kb, &metadata, || {
        move_noclobber(&source, &receipt.destination)
    })?;
    Ok(receipt)
}

/// Read only native file-URL pasteboard objects on the main thread; path text is never interpreted.
/// Returns an empty list when the pasteboard has no file URLs and never executes clipboard content.
pub fn read_files_from_clipboard() -> Result<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        native::read_files()
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Native file paste requires macOS")
    }
}

/// Resolve a real owned folder while stopping at its own nested-root marker.
fn owned_folder(kb: &KnowledgeBase, relative: &str) -> Result<PathBuf> {
    let directory = if relative.is_empty() {
        kb.root.clone()
    } else {
        ensure_no_internal_components(Path::new(relative))?;
        let directory = kb.resolve(relative)?;
        ensure_owned_directories(kb, &directory)?;
        directory
    };
    ensure!(
        fs::symlink_metadata(&directory)?.is_dir(),
        "Destination must be a real folder"
    );
    let canonical = directory.canonicalize()?;
    ensure!(
        canonical.starts_with(&kb.root),
        "Folder is outside the knowledge root"
    );
    Ok(canonical)
}

/// Convert an already contained live path to its exact metadata identity.
fn root_relative(kb: &KnowledgeBase, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(&kb.root)?
        .to_str()
        .context("Source path is not UTF-8")?
        .to_owned())
}

/// Reject knowledge internals wherever they occur in a selected relative path.
fn ensure_no_internal_components(path: &Path) -> Result<()> {
    ensure!(
        !path.components().any(
            |part| matches!(part, Component::Normal(name) if name == ".omni" || name == ".git")
        ),
        "Internal state is not a source action target"
    );
    Ok(())
}

/// Treat any marker entry as an ownership boundary, including a malformed or dangling marker.
fn ensure_no_marker(directory: &Path) -> Result<()> {
    ensure!(
        !crate::storage::has_marker(directory)?,
        "Folder belongs to a nested knowledge root"
    );
    Ok(())
}

/// Check every owned directory ancestor, not just the selected folder, for marker boundaries.
fn ensure_owned_directories(kb: &KnowledgeBase, directory: &Path) -> Result<()> {
    for ancestor in directory.ancestors() {
        if ancestor == kb.root {
            return Ok(());
        }
        ensure_no_marker(ancestor)?;
    }
    bail!("Directory is outside the knowledge root")
}

/// Validate external regular sources and reuse editable metadata only for files owned by this root.
fn copy_source(kb: &KnowledgeBase, source: &Path) -> Result<(PathBuf, Option<AssetMetadata>)> {
    ensure_no_internal_components(source)?;
    let metadata = fs::symlink_metadata(source).context("Cannot inspect copy source")?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Copy source must be a regular file without a symlink"
    );
    if let Ok(relative) = source.strip_prefix(&kb.root) {
        owned_source(kb, relative.to_str().context("Source path is not UTF-8")?)?;
    }
    let canonical = source.canonicalize()?;
    if canonical.starts_with(&kb.root) {
        let relative = root_relative(kb, &canonical)?;
        let path = owned_source(kb, &relative)?;
        Ok((path, Some(kb.read_metadata(&relative)?)))
    } else {
        Ok((canonical, None))
    }
}

/// Prepare a destination filename and refuse source, directory, or metadata collisions before writes.
fn transfer_destination(
    kb: &KnowledgeBase,
    source: &Path,
    folder: &str,
) -> Result<TransferReceipt> {
    let directory = owned_folder(kb, folder)?;
    let name = source.file_name().context("Source has no filename")?;
    ensure!(
        name != CONFIG_FILE && name != ".omni" && name != ".git",
        "Reserved source filename cannot be transferred"
    );
    let destination = directory.join(name);
    let relative = root_relative(kb, &destination)?;
    kb.owned_path(&relative, false)?;
    ensure_missing(
        &destination,
        "Destination already exists; no file was replaced",
    )?;
    ensure_missing(
        &kb.metadata_path(&relative)?,
        "Destination metadata already exists; authored notes were preserved",
    )?;
    Ok(TransferReceipt {
        source: source.into(),
        destination,
        relative,
    })
}

/// Distinguish absence from permission errors and dangling symlinks, all of which must not be clobbered.
fn ensure_missing(path: &Path, collision: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Cannot inspect transfer destination"),
        Ok(_) => bail!("{collision}: {}", path.display()),
    }
}

/// Copy only authored fields and assign their new path identity without merging unrelated records.
fn destination_metadata(relative: &str, original: Option<AssetMetadata>) -> AssetMetadata {
    let mut metadata = original.unwrap_or_else(|| AssetMetadata {
        version: 1,
        path: String::new(),
        tags: vec![],
        description: String::new(),
    });
    metadata.path = relative.into();
    metadata
}

/// Open a source without following a final symlink or blocking on a substituted special file.
fn open_regular_source(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Copy source changed to a nonregular file"
    );
    Ok(file)
}

/// Reserve a new sidecar before the file effect; rollback removes only this operation's own inode.
/// A process crash can leave that new sidecar for recovery; retries refuse to overwrite it.
fn commit_transfer(
    kb: &KnowledgeBase,
    metadata: &AssetMetadata,
    effect: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let path = kb.metadata_path(&metadata.path)?;
    ensure_missing(
        &path,
        "Destination metadata already exists; authored notes were preserved",
    )?;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    serde_json::to_writer_pretty(&mut temporary, metadata)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    let original = MetadataFingerprint::capture(temporary.as_file_mut())?;
    let reservation = temporary
        .persist_noclobber(&path)
        .map_err(|error| error.error)
        .context("Destination metadata could not be reserved without replacement")?;
    if let Err(error) = effect() {
        let retained = remove_created_metadata(&path, &original);
        drop(reservation);
        return match retained {
            Ok(()) => Err(error),
            Err(cleanup) => Err(error).context(format!(
                "Destination metadata retained for recovery at {}: {cleanup:#}",
                path.display()
            )),
        };
    }
    Ok(())
}

/// Identity and content observed before publication; an in-place edit must not be cleaned up.
struct MetadataFingerprint {
    /// Device that owns the reserved inode.
    device: u64,
    /// Inode pinned by the retained reservation file.
    inode: u64,
    /// Original byte count, checked before reading potentially replaced state.
    length: u64,
    /// Original bytes hashed before the file transfer effect.
    digest: [u8; 32],
}

impl MetadataFingerprint {
    /// Hash a regular reserved record with fixed memory while retaining its pre-effect inode identity.
    fn capture(file: &mut File) -> Result<Self> {
        let metadata = file.metadata()?;
        file.rewind()?;
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        let mut buffer = [0; 16 * 1024];
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            digest.update(&buffer[..length]);
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            digest: digest.finalize().into(),
        })
    }
}

/// Refuse to remove a sidecar replaced by another actor, even when an effect failed.
fn remove_created_metadata(path: &Path, original: &MetadataFingerprint) -> Result<()> {
    let current = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        current.is_file()
            && !current.file_type().is_symlink()
            && current.dev() == original.device
            && current.ino() == original.inode
            && current.len() == original.length,
        "Sidecar identity changed; unrelated state was preserved"
    );
    let mut current_file = open_regular_source(path)?;
    let current = MetadataFingerprint::capture(&mut current_file)?;
    ensure!(
        current.device == original.device
            && current.inode == original.inode
            && current.length == original.length
            && current.digest == original.digest,
        "Sidecar contents changed; authored edits were preserved"
    );
    fs::remove_file(path).context("Cannot remove this operation's reserved metadata")
}

/// Use the Darwin kernel's exclusive rename instead of a check followed by an overwriting rename.
fn move_noclobber(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())?;
        let destination = CString::new(destination.as_os_str().as_bytes())?;
        // SAFETY: Both pointers remain valid NUL-terminated path strings for this synchronous call.
        if unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) }
            != 0
        {
            return Err(std::io::Error::last_os_error()).context("Move failed without replacing the destination; cross-filesystem moves require Copy");
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source, destination);
        bail!("Atomic exclusive moves require macOS")
    }
}

/// Resolve an existing owned regular file; reject markers, internals, nested roots, and symlinks.
/// Returns the absolute source path without reading its contents or changing metadata.
pub fn source_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let kb = open_exact_root(root)?;
    owned_source(&kb, relative)
}

/// Validate the source again at the effect boundary, including reserved path components.
fn owned_source(kb: &KnowledgeBase, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    ensure!(
        relative != CONFIG_FILE,
        "The knowledge root marker is not a source action target"
    );
    ensure_no_internal_components(path)?;
    let absolute = kb.resolve(relative)?;
    let metadata = fs::symlink_metadata(&absolute).context("Cannot inspect selected source")?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Source actions require a regular file"
    );
    ensure_owned_directories(
        kb,
        absolute
            .parent()
            .context("Source has no parent directory")?,
    )?;
    let canonical = absolute.canonicalize()?;
    ensure!(
        canonical.starts_with(&kb.root),
        "Source is outside the knowledge root"
    );
    Ok(canonical)
}

/// Ask Finder to reveal the selected source using typed process arguments.
/// Run on a worker; this waits at most ten seconds for dispatch, not for Finder interaction.
pub fn reveal(root: &Path, relative: &str) -> Result<()> {
    let source = source_path(root, relative)?;
    dispatch_open(Some("-R"), &source)
}

/// Reveal the saved authored JSON for an owned source after revalidating its exact root and sidecar.
/// Returns an error for missing or invalid metadata and unsafe source identities without opening a path.
/// Run on a worker; dispatch waits at most ten seconds and does not confirm subsequent Finder interaction.
pub fn reveal_metadata(root: &Path, relative: &str) -> Result<()> {
    let path = metadata_reveal_path(root, relative)?;
    dispatch_open(Some("-R"), &path)
}

/// Resolve a real authored sidecar through live source validation without an index read or native effect.
fn metadata_reveal_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let info = crate::file_info::source_info(root, relative)?;
    ensure!(
        info.metadata_saved,
        "No saved metadata exists for this file"
    );
    Ok(info.metadata_path)
}

/// Open an owned source with its installed default macOS application, using typed arguments.
/// Run on a worker; the result confirms launcher dispatch, not that the target application rendered it.
pub fn open_original(root: &Path, relative: &str) -> Result<()> {
    let source = source_path(root, relative)?;
    dispatch_open(None, &source)
}

/// Request the system Quick Look viewer for an owned source, including unsupported inline formats.
/// Returns launch acceptance only; a background waiter reaps the viewer after it closes.
/// The visible viewer continues independently if this application exits; no deadline closes it.
pub fn quick_look(root: &Path, relative: &str) -> Result<()> {
    ensure!(
        cfg!(target_os = "macos"),
        "System Quick Look requires macOS"
    );
    let source = source_path(root, relative)?;
    let child = quick_look_command(&source)
        .spawn()
        .context("Cannot launch system Quick Look")?;
    let owned = std::sync::Arc::new(std::sync::Mutex::new(child));
    let reaper = owned.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("omni-quick-look-reaper".into())
        .spawn(move || {
            if let Ok(mut child) = reaper.lock() {
                let _ = child.wait();
            }
        })
    {
        if let Ok(mut child) = owned.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        return Err(error).context("Cannot supervise Quick Look; the launched viewer was closed");
    }
    Ok(())
}

/// Build a fixed Quick Look request without a shell, captured content, or a viewer timeout.
fn quick_look_command(path: &Path) -> Command {
    let mut command = Command::new("/usr/bin/qlmanage");
    command
        .arg("-p")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Open the existing canonical root folder without initialization or index changes.
/// Run on a worker; a dispatch failure does not prove Finder never received the request.
pub fn open_root(root: &Path) -> Result<()> {
    let canonical = root
        .canonicalize()
        .context("The knowledge folder is unavailable")?;
    ensure!(canonical.is_dir(), "The knowledge location is not a folder");
    dispatch_open(None, &canonical)
}

/// Write a native file URL to the general pasteboard so Finder can paste the source file.
/// Must be called on the native main thread after a user action; path-text copying is separate.
pub fn copy_file(root: &Path, relative: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        native::copy_file(root, relative)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (root, relative);
        bail!("Native file copying requires macOS")
    }
}

/// Move one selected regular file to macOS Trash after the caller obtains confirmation.
/// Takes the cooperative writer gate and revalidates ownership immediately before the effect.
/// Keeps authored sidecar metadata and the published index; callers request reconciliation explicitly.
pub fn trash_file(root: &Path, relative: &str) -> Result<TrashReceipt> {
    trash_with(root, relative, native_trash)
}

/// Serialize the destructive boundary and permit an isolated adapter test without host Trash effects.
fn trash_with(
    root: &Path,
    relative: &str,
    effect: impl FnOnce(&Path) -> Result<Option<PathBuf>>,
) -> Result<TrashReceipt> {
    let kb = open_exact_root(root)?;
    let _writer = kb.writer_lock()?;
    let source = owned_source(&kb, relative)?;
    let trashed_path = effect(&source)?;
    Ok(TrashReceipt {
        source,
        trashed_path,
    })
}

/// Perform only the platform Trash action; there is no permanent-delete fallback.
fn native_trash(source: &Path) -> Result<Option<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        native::trash(source)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = source;
        bail!("Native Trash requires macOS")
    }
}

/// Dispatch one fixed macOS opener with bounded diagnostics and an owned launcher deadline.
fn dispatch_open(option: Option<&str>, path: &Path) -> Result<()> {
    ensure!(cfg!(target_os = "macos"), "Finder actions require macOS");
    let mut diagnostics = tempfile::tempfile().context("Cannot prepare Finder diagnostics")?;
    let mut command = open_command(option, path);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(diagnostics.try_clone()?)
        .spawn()
        .context("Cannot launch Finder request")?;
    let start = Instant::now();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break exit,
            Ok(None) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!(
                    "Finder dispatch was not confirmed within ten seconds; Finder may already have received the request"
                )
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context(
                    "Cannot confirm Finder dispatch; the request may already have been received",
                );
            }
        }
    };
    if !exit.success() {
        diagnostics.rewind()?;
        let mut message = String::new();
        diagnostics.take(16 * 1024).read_to_string(&mut message)?;
        bail!("Finder dispatch failed ({exit}): {}", message.trim());
    }
    Ok(())
}

/// Construct only the fixed system opener; filenames remain single arguments even with shell syntax.
fn open_command(option: Option<&str>, path: &Path) -> Command {
    let mut command = Command::new("/usr/bin/open");
    if let Some(option) = option {
        command.arg(option);
    }
    command.arg(path);
    command
}

/// Objective-C objects stay inside a per-call autorelease pool and do not cross workers.
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use objc2::{ClassType, rc::autoreleasepool, runtime::ProtocolObject};
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSFileManager, NSString, NSThread, NSURL};

    /// Publish an NSURL through NSPasteboardWriting rather than a plain path string.
    fn copy_file_inner(source: &Path) -> Result<()> {
        let path = NSString::from_str(source.to_str().context("Source path is not UTF-8")?);
        let url = NSURL::fileURLWithPath_isDirectory(&path, false);
        let object = ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&*url);
        let objects = NSArray::from_slice(&[object]);
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        ensure!(
            pasteboard.writeObjects(&objects),
            "macOS did not accept the source file on the pasteboard"
        );
        Ok(())
    }

    /// Reject worker callers before the general pasteboard is opened or cleared.
    pub(super) fn copy_file(root: &Path, relative: &str) -> Result<()> {
        main_thread_only(|| {
            let source = source_path(root, relative)?;
            autoreleasepool(|_| copy_file_inner(&source))
        })
    }

    /// Guard both clipboard boundaries without requiring host clipboard effects in regression tests.
    pub(super) fn main_thread_only<T>(effect: impl FnOnce() -> Result<T>) -> Result<T> {
        ensure!(
            NSThread::isMainThread_class(),
            "File clipboard actions must run on the native main thread"
        );
        effect()
    }

    /// Read class-filtered NSURL objects and return only local file paths, without interpreting text.
    pub(super) fn read_files() -> Result<Vec<PathBuf>> {
        main_thread_only(|| {
            autoreleasepool(|_| {
                let classes = NSArray::from_slice(&[NSURL::class()]);
                let pasteboard = NSPasteboard::generalPasteboard();
                // SAFETY: This is AppKit's immutable, process-lifetime file-URL type identifier.
                let file_url_type = unsafe { NSPasteboardTypeFileURL };
                if !pasteboard
                    .types()
                    .is_some_and(|types| types.containsObject(file_url_type))
                {
                    return Ok(Vec::new());
                }
                // SAFETY: The requested class array contains only NSURL; there are no untyped option values.
                let objects = unsafe { pasteboard.readObjectsForClasses_options(&classes, None) };
                let mut paths = Vec::new();
                if let Some(objects) = objects {
                    for index in 0..objects.count() {
                        let object = objects.objectAtIndex(index);
                        if let Some(url) = object.downcast_ref::<NSURL>() {
                            let local_host = url.host().is_none_or(|host| {
                                host.is_empty() || host.to_string() == "localhost"
                            });
                            if url.isFileURL()
                                && local_host
                                && let Some(path) = url.path()
                            {
                                paths.push(PathBuf::from(path.to_string()));
                            }
                        }
                    }
                }
                Ok(paths)
            })
        })
    }

    /// Move with NSFileManager.trashItemAtURL and retain any actual resulting file URL.
    /// Apple API: <https://developer.apple.com/documentation/foundation/filemanager/trashitem(at:resultingitemurl:)>
    pub(super) fn trash(source: &Path) -> Result<Option<PathBuf>> {
        autoreleasepool(|_| {
            let path = NSString::from_str(source.to_str().context("Source path is not UTF-8")?);
            let url = NSURL::fileURLWithPath_isDirectory(&path, false);
            let mut resulting_url = None;
            NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting_url))
                .map_err(|error| {
                    anyhow::anyhow!(
                        "macOS could not move the source to Trash: {}",
                        error.localizedDescription()
                    )
                })?;
            Ok(resulting_url
                .and_then(|url| url.path())
                .map(|path| PathBuf::from(path.to_string())))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate as fixture_library;
    mod retained {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/retained.rs"
        ));
    }
    use super::*;
    use fs2::FileExt;
    use retained::legacy_init;
    use std::fs::OpenOptions;

    /// Resolve only a real saved record for the validated source; do not launch Finder or publish an index.
    #[test]
    fn metadata_reveal_requires_a_saved_record_and_preserves_its_identity() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Metadata reveal")?;
        let relative = "raw/资料 + #1.txt";
        fs::write(kb.root.join(relative), "source")?;
        let missing = metadata_reveal_path(&kb.root, relative).unwrap_err();
        assert!(format!("{missing:#}").contains("No saved metadata"));
        kb.set_metadata(relative, &["keep".into()], Some("Saved notes"))?;
        let expected = kb.metadata_path(relative)?;
        let original = fs::read(&expected)?;
        let path = metadata_reveal_path(&kb.root, "raw/./资料 + #1.txt")?;
        assert_eq!(path, expected);
        assert_eq!(fs::read(path)?, original);
        assert_eq!(fs::read_to_string(kb.root.join(relative))?, "source");
        assert!(!kb.root.join(".omni/current.json").exists());
        Ok(())
    }

    /// Reject unsafe identities, nested ownership, symlink sidecars, and removed exact roots before dispatch.
    #[test]
    fn metadata_reveal_revalidates_source_and_sidecar_containment() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let parent = legacy_init(directory.path(), "Parent")?;
        let child = legacy_init(&parent.root.join("child"), "Child")?;
        let relative = "raw/source.txt";
        fs::write(parent.root.join(relative), "parent source")?;
        fs::write(child.root.join(relative), "child source")?;
        parent.set_metadata(relative, &[], Some("Parent notes"))?;
        child.set_metadata(relative, &[], Some("Child notes"))?;
        for rejected in [
            "../outside",
            CONFIG_FILE,
            ".omni/write.lock",
            "raw",
            "raw/missing",
            "child/raw/source.txt",
        ] {
            assert!(metadata_reveal_path(&parent.root, rejected).is_err());
        }
        #[cfg(unix)]
        {
            let sidecar = parent.metadata_path(relative)?;
            let outside = directory.path().join("outside.json");
            fs::rename(&sidecar, &outside)?;
            std::os::unix::fs::symlink(&outside, &sidecar)?;
            assert!(metadata_reveal_path(&parent.root, relative).is_err());
            assert!(outside.is_file());
        }
        fs::remove_file(child.root.join(CONFIG_FILE))?;
        assert!(metadata_reveal_path(&child.root, relative).is_err());
        Ok(())
    }

    /// Native launch commands are inspected without launching any host application or test helper.
    #[test]
    fn native_launch_arguments_preserve_the_exact_file_path() {
        use std::ffi::OsStr;
        let path = Path::new("/library/中文 ' $() #.RAF");
        let reveal = open_command(Some("-R"), path);
        assert_eq!(reveal.get_program(), OsStr::new("/usr/bin/open"));
        assert_eq!(
            reveal.get_args().collect::<Vec<_>>(),
            [OsStr::new("-R"), path.as_os_str()]
        );
        let open = open_command(None, path);
        assert_eq!(open.get_args().collect::<Vec<_>>(), [path.as_os_str()]);
        let preview = quick_look_command(path);
        assert_eq!(preview.get_program(), OsStr::new("/usr/bin/qlmanage"));
        assert_eq!(
            preview.get_args().collect::<Vec<_>>(),
            [OsStr::new("-p"), path.as_os_str()]
        );
    }

    /// Worker rejection is tested with an inert callback, so even a broken guard cannot access the clipboard.
    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_guard_refuses_a_background_effect() {
        let error = std::thread::spawn(|| {
            native::main_thread_only(|| -> Result<()> {
                bail!("unexpected clipboard effect callback")
            })
        })
        .join()
        .expect("main-thread guard worker must not panic")
        .unwrap_err();
        assert!(format!("{error:#}").contains("main thread"));
    }

    /// A busy writer is tested with an inert Trash adapter, never with the host Trash API.
    #[test]
    fn trash_guard_refuses_a_competing_writer() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Actions")?;
        fs::write(kb.root.join("raw/source.txt"), "source")?;
        kb.set_metadata("raw/source.txt", &["keep".into()], Some("notes"))?;
        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(kb.root.join(".omni/write.lock"))?;
        gate.lock_exclusive()?;
        let error = trash_with(&kb.root, "raw/source.txt", |_| {
            bail!("unexpected native effect callback")
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("another library writer"));
        assert_eq!(
            fs::read_to_string(kb.root.join("raw/source.txt"))?,
            "source"
        );
        Ok(())
    }

    /// Failure after sidecar reservation removes only the new record and preserves authored originals.
    #[test]
    fn failed_transfer_rolls_back_its_reserved_metadata() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Transfer")?;
        fs::write(kb.root.join("raw/source.txt"), "source")?;
        kb.set_metadata("raw/source.txt", &["keep".into()], Some("notes"))?;
        let original_path = kb.metadata_path("raw/source.txt")?;
        let original = fs::read(&original_path)?;
        let metadata = destination_metadata("new.txt", Some(kb.read_metadata("raw/source.txt")?));
        let reservation = kb.metadata_path("new.txt")?;
        let _writer = kb.writer_lock()?;
        let error = commit_transfer(&kb, &metadata, || {
            assert!(
                reservation.is_file(),
                "metadata must be reserved before the file effect"
            );
            bail!("simulated file publication failure")
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("simulated file publication failure"));
        assert!(!reservation.exists());
        assert_eq!(fs::read(original_path)?, original);
        assert_eq!(
            fs::read_to_string(kb.root.join("raw/source.txt"))?,
            "source"
        );
        Ok(())
    }

    /// An external replacement defeats cleanup; the new actor's sidecar remains with an explicit residual path.
    #[test]
    fn failed_transfer_keeps_a_replaced_sidecar() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Transfer")?;
        let metadata = destination_metadata("new.txt", None);
        let reservation = kb.metadata_path("new.txt")?;
        let _writer = kb.writer_lock()?;
        let error = commit_transfer(&kb, &metadata, || {
            let replacement = directory.path().join("replacement.json");
            fs::write(&replacement, "unrelated authored notes")?;
            fs::rename(replacement, &reservation)?;
            bail!("simulated file publication failure")
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("retained for recovery"));
        assert_eq!(fs::read_to_string(reservation)?, "unrelated authored notes");
        Ok(())
    }

    /// A same-inode edit must be retained; comparing only inode ownership would erase authored changes.
    #[test]
    fn failed_transfer_keeps_an_in_place_sidecar_edit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Transfer")?;
        let mut metadata = destination_metadata("new.txt", None);
        metadata.description = "before".into();
        let reservation = kb.metadata_path("new.txt")?;
        let _writer = kb.writer_lock()?;
        let error = commit_transfer(&kb, &metadata, || {
            let original = fs::symlink_metadata(&reservation)?;
            let mut bytes = fs::read(&reservation)?;
            let position = bytes
                .windows(b"before".len())
                .position(|value| value == b"before")
                .expect("authored description must be present");
            bytes[position] = b'B';
            fs::write(&reservation, &bytes)?;
            let edited = fs::symlink_metadata(&reservation)?;
            assert_eq!(original.ino(), edited.ino());
            assert_eq!(original.len(), edited.len());
            bail!("simulated file publication failure")
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("retained for recovery"));
        assert!(fs::read_to_string(reservation)?.contains("Before"));
        Ok(())
    }

    /// A destination appearing after preflight is protected by the actual Darwin exclusive-rename flag.
    #[cfg(target_os = "macos")]
    #[test]
    fn exclusive_move_refuses_a_late_collision() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("source");
        let destination = directory.path().join("destination");
        fs::write(&source, "source")?;
        fs::write(&destination, "unrelated")?;
        assert!(move_noclobber(&source, &destination).is_err());
        assert_eq!(fs::read_to_string(source)?, "source");
        assert_eq!(fs::read_to_string(destination)?, "unrelated");
        Ok(())
    }

    /// An isolated fake moves only its fixture and observes the writer gate and retained authored state.
    #[test]
    fn trash_adapter_preserves_metadata_and_published_generation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Actions")?;
        let relative = "raw/source.txt";
        fs::write(kb.root.join(relative), "source")?;
        kb.set_metadata(relative, &["keep".into()], Some("authored notes"))?;
        kb.index(&crate::IndexOptions::default())?;
        let pointer = fs::read(kb.root.join(".omni/current.json"))?;
        let metadata_path = kb.metadata_path(relative)?;
        let metadata = fs::read(&metadata_path)?;
        let destination = directory.path().join("fixture-trash.txt");
        let receipt = trash_with(&kb.root, relative, |source| {
            let gate = OpenOptions::new()
                .read(true)
                .write(true)
                .open(kb.root.join(".omni/write.lock"))?;
            assert!(
                gate.try_lock_exclusive().is_err(),
                "Trash must retain the cooperative writer gate"
            );
            fs::rename(source, &destination)?;
            Ok(Some(destination.clone()))
        })?;
        assert_eq!(receipt.source, kb.root.join(relative));
        assert_eq!(receipt.trashed_path, Some(destination.clone()));
        assert!(!kb.root.join(relative).exists());
        assert_eq!(fs::read_to_string(destination)?, "source");
        assert_eq!(fs::read(metadata_path)?, metadata);
        assert_eq!(fs::read(kb.root.join(".omni/current.json"))?, pointer);
        Ok(())
    }

    /// Refuse a removed child root before its same-named parent file can reach a native effect adapter.
    #[test]
    fn trash_adapter_refuses_ancestor_rebinding() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let parent = legacy_init(directory.path(), "Parent")?;
        let child = legacy_init(&parent.root.join("child"), "Child")?;
        let relative = "raw/source.txt";
        fs::write(parent.root.join(relative), "parent source")?;
        fs::write(child.root.join(relative), "child source")?;
        fs::remove_file(child.root.join(CONFIG_FILE))?;
        assert_eq!(KnowledgeBase::open(&child.root)?.root, parent.root);
        let called = std::cell::Cell::new(false);
        let error = trash_with(&child.root, relative, |_| {
            called.set(true);
            bail!("inert adapter must not be called for a changed selected root")
        })
        .unwrap_err();
        assert!(!called.get());
        assert!(format!("{error:#}").contains("Selected knowledge root"));
        assert_eq!(
            fs::read_to_string(parent.root.join(relative))?,
            "parent source"
        );
        assert_eq!(
            fs::read_to_string(child.root.join(relative))?,
            "child source"
        );
        Ok(())
    }

    /// Report successful movement without inventing a destination when the native API omits its URL.
    #[test]
    fn trash_receipt_allows_an_unreported_destination() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Actions")?;
        fs::write(kb.root.join("raw/source.txt"), "source")?;
        let destination = directory.path().join("fixture-trash.txt");
        let receipt = trash_with(&kb.root, "raw/source.txt", |source| {
            fs::rename(source, &destination)?;
            Ok(None)
        })?;
        assert_eq!(receipt.source, kb.root.join("raw/source.txt"));
        assert!(receipt.trashed_path.is_none());
        assert!(!kb.root.join("raw/source.txt").exists());
        assert_eq!(fs::read_to_string(destination)?, "source");
        Ok(())
    }

    /// A native refusal propagates without running a permanent-delete fallback or changing authored notes.
    #[test]
    fn trash_adapter_failure_preserves_the_source() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = legacy_init(directory.path(), "Actions")?;
        let relative = "raw/source.txt";
        fs::write(kb.root.join(relative), "source")?;
        kb.set_metadata(relative, &["keep".into()], Some("authored notes"))?;
        let metadata_path = kb.metadata_path(relative)?;
        let metadata = fs::read(&metadata_path)?;
        let error =
            trash_with(&kb.root, relative, |_| bail!("simulated native refusal")).unwrap_err();
        assert!(format!("{error:#}").contains("simulated native refusal"));
        assert_eq!(fs::read_to_string(kb.root.join(relative))?, "source");
        assert_eq!(fs::read(metadata_path)?, metadata);
        Ok(())
    }
}
