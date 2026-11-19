//! One actual-root qualified selector over complete existing-engine tuples, not another index.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Pointer {
    pub format: u32,
    pub library: String,
    pub original_root: (u64, u64),
    pub stage: String,
    pub stage_root: (u64, u64),
    pub header_version: [u64; 8],
    pub header_sha256: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Unselected,
    Candidate,
    Switched,
    Durable,
}
struct Candidate {
    name: String,
    file: File,
    bytes: Vec<u8>,
    pointer: Pointer,
}
// No explicit flock unlock in Drop: inherited descriptors are never parent settlement authority.
pub(super) struct Selection {
    parent: File,
    namespace: File,
    namespace_name: String,
    source: File,
    writer: File,
    owner_pid: u32,
    expected: Option<(File, [u64; 8])>,
    candidate: Option<Candidate>,
    writer_locked: bool,
    pub phase: Phase,
}
fn identity(file: &File) -> Result<(u64, u64)> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}
fn named(directory: &File, name: &str) -> Result<Option<File>> {
    match rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            anyhow::ensure!(file.metadata()?.is_file(), "nonregular tuple selector");
            Ok(Some(file))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn namespace_name(library: &str, root: (u64, u64)) -> String {
    let mut hash = Sha256::new();
    hash.update(b"omni-native-complete-tuple-v1\0");
    hash.update((library.len() as u64).to_le_bytes());
    hash.update(library.as_bytes());
    hash.update(root.0.to_le_bytes());
    hash.update(root.1.to_le_bytes());
    format!("native-tuples-{}", hex::encode(hash.finalize()))
}
impl Selection {
    pub fn directory(&self) -> Result<File> {
        self.cache_check()?;
        Ok(self.namespace.try_clone()?)
    }
    pub fn path(&self, generated: &Path) -> PathBuf {
        generated.join(&self.namespace_name)
    }
    pub fn open(owner: &NativeCorpusDraft) -> Result<Self> {
        owner.check()?;
        let wire::Command::Refresh { library_id, .. } =
            owner.job.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Refresh selector write intent required")
        };
        let source = owner
            .job
            .permit
            .with_selected_context(library_id, |root, _, _| root.try_clone().map_err(refused))
            .map_err(refused_error)?;
        let root = identity(&source)?;
        anyhow::ensure!(
            root == owner.job.cause.inventory.owner.root,
            "selector original root differs from actual M1 owner"
        );
        let namespace_name = namespace_name(library_id.as_str(), root);
        let parent = Account::current()?.generated_directory()?;
        match rustix::fs::mkdirat(
            &parent,
            namespace_name.as_str(),
            rustix::fs::Mode::from_bits_truncate(0o700),
        ) {
            Ok(()) => parent.sync_all()?,
            Err(rustix::io::Errno::EXIST) => {}
            Err(e) => return Err(e.into()),
        }
        let namespace = File::from(rustix::fs::openat(
            &parent,
            namespace_name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        let writer = File::from(rustix::fs::openat(
            &namespace,
            "WRITER",
            rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::from_bits_truncate(0o600),
        )?);
        let expected = named(&namespace, "CURRENT")?
            .map(|file| stamp(&file).map(|version| (file, version)))
            .transpose()?;
        let state = Self {
            parent,
            namespace,
            namespace_name,
            source,
            writer,
            owner_pid: std::process::id(),
            expected,
            candidate: None,
            writer_locked: false,
            phase: Phase::Unselected,
        };
        state.check(owner)?;
        state.check_expected()?;
        Ok(state)
    }
    fn cache_check(&self) -> Result<()> {
        anyhow::ensure!(
            self.owner_pid == std::process::id(),
            "tuple writer belongs to original process"
        );
        anyhow::ensure!(
            identity(&Account::current()?.generated_directory()?)? == identity(&self.parent)?,
            "tuple generated owner changed"
        );
        let now = File::from(rustix::fs::openat(
            &self.parent,
            self.namespace_name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        anyhow::ensure!(
            identity(&now)? == identity(&self.namespace)?,
            "tuple selector namespace changed"
        );
        let now =
            named(&self.namespace, "WRITER")?.context("tuple writer descriptor disappeared")?;
        anyhow::ensure!(
            identity(&now)? == identity(&self.writer)?,
            "tuple writer identity changed"
        );
        Ok(())
    }
    pub fn check(&self, owner: &NativeCorpusDraft) -> Result<()> {
        self.cache_check()?;
        let actual = owner
            .job
            .permit
            .with_selected_context(&owner.job.cause.library, |root, _, _| {
                root.metadata().map_err(refused)
            })
            .map_err(refused_error)?;
        anyhow::ensure!(
            (actual.dev(), actual.ino()) == identity(&self.source)?,
            "tuple selected source owner changed"
        );
        Ok(())
    }
    fn check_expected(&self) -> Result<()> {
        let current = named(&self.namespace, "CURRENT")?;
        match (&self.expected, current) {
            (None, None) => Ok(()),
            (Some((held, version)), Some(current)) => {
                anyhow::ensure!(
                    stamp(held)? == *version && stamp(&current)? == *version,
                    "whole tuple selector CAS conflict"
                );
                Ok(())
            }
            _ => bail!("whole tuple selector CAS conflict"),
        }
    }
    fn acquire(
        &mut self,
        owner: &NativeCorpusDraft,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        while !self.writer_locked {
            owner.check()?;
            self.check(owner)?;
            match fs2::FileExt::try_lock_exclusive(&self.writer) {
                Ok(()) => self.writer_locked = true,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    progress(NativeCorpusProgress::WriterWaiting);
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    pub fn select(
        &mut self,
        owner: &mut NativeCorpusDraft,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        if self.phase == Phase::Switched {
            return self.recover(owner);
        }
        owner.check()?;
        self.check(owner)?;
        if self.phase == Phase::Durable {
            return self.check_selected();
        }
        anyhow::ensure!(
            self.phase == Phase::Unselected,
            "existing selector candidate must be explicitly recovered/disposed before retry"
        );
        self.acquire(owner, progress)?;
        self.check_expected()?;
        let seal = owner
            .tuple
            .as_ref()
            .context("complete sealed tuple required")?;
        seal.verify_members(
            owner.files.as_ref().context("actual complete tuple")?,
            || owner.check(),
        )?;
        let files = owner.files.as_ref().context("actual complete tuple")?;
        let stage = files
            .stage
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .context("native tuple generation name")?
            .to_owned();
        anyhow::ensure!(
            stage.starts_with("native-corpus-") && !stage.contains('/') && !stage.contains('\\'),
            "invalid generated tuple name"
        );
        let pointer = Pointer {
            format: 1,
            library: owner.job.cause.library.as_str().to_owned(),
            original_root: identity(&self.source)?,
            stage,
            stage_root: identity(&files.directory)?,
            header_version: seal.header_version,
            header_sha256: tuple::hash(&seal.header, || owner.check())?,
        };
        let bytes = serde_json::to_vec(&pointer)?;
        anyhow::ensure!(bytes.len() <= FRAME, "tuple selector exceeds native frame");
        let name = format!("select-{}", uuid::Uuid::new_v4());
        let file = File::from(rustix::fs::openat(
            &self.namespace,
            name.as_str(),
            rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::from_bits_truncate(0o600),
        )?);
        self.candidate = Some(Candidate {
            name,
            file,
            bytes,
            pointer,
        });
        self.phase = Phase::Candidate;
        let length = self
            .candidate
            .as_ref()
            .expect("actual candidate")
            .bytes
            .len() as u64;
        owner.wait_growth(length, None, progress)?;
        self.check(owner)?;
        owner.check()?;
        let candidate = self.candidate.as_mut().expect("actual candidate");
        let before = candidate.file.metadata()?.len();
        let result = candidate.file.write_all(&candidate.bytes);
        let after = candidate.file.metadata()?.len();
        owner
            .work
            .as_mut()
            .context("actual selector metadata loan")?
            .written(
                after
                    .checked_sub(before)
                    .context("selector size regressed")?,
            )?;
        result?;
        candidate.file.sync_all()?;
        owner.check()?;
        self.check(owner)?;
        self.check_expected()?;
        let candidate = self.candidate.as_ref().expect("actual candidate");
        rustix::fs::renameat(
            &self.namespace,
            candidate.name.as_str(),
            &self.namespace,
            "CURRENT",
        )?;
        self.phase = Phase::Switched; // Must precede every fallible operation after the atomic switch.
        #[cfg(test)]
        if let Some(hook) = owner.test_after_switch.take() {
            hook();
        }
        #[cfg(test)]
        if owner.test_fail_after_switch {
            owner.test_fail_after_switch = false;
            bail!("injected post-switch durability observation failure");
        }
        self.recover(owner)
    }
    fn check_selected(&self) -> Result<()> {
        self.cache_check()?;
        let candidate = self
            .candidate
            .as_ref()
            .context("actual switched selector owner")?;
        let current =
            named(&self.namespace, "CURRENT")?.context("selected tuple pointer disappeared")?;
        anyhow::ensure!(
            identity(&current)? == identity(&candidate.file)?,
            "selected tuple pointer identity changed"
        );
        let wanted = hex::encode(Sha256::digest(&candidate.bytes));
        anyhow::ensure!(
            tuple::hash(&candidate.file, || self.cache_check())? == wanted,
            "selected tuple pointer content changed"
        );
        Ok(())
    }
    // Settlement of an already-started atomic switch, not a new source/body/publication permission.
    pub fn recover(&mut self, owner: &mut NativeCorpusDraft) -> Result<()> {
        anyhow::ensure!(
            self.writer_locked && matches!(self.phase, Phase::Switched | Phase::Durable),
            "no actual pending selector switch"
        );
        self.check_selected()?;
        self.namespace.sync_all()?;
        let files = owner
            .files
            .as_mut()
            .context("actual selected generation owner")?;
        let candidate = self
            .candidate
            .as_ref()
            .context("actual switched selector")?;
        anyhow::ensure!(
            identity(&files.directory)? == candidate.pointer.stage_root,
            "selected actual generation changed"
        );
        files.check()?;
        files.stage.adopt();
        self.phase = Phase::Durable;
        Ok(())
    }
    pub fn discard_candidate(&mut self) -> Result<()> {
        if self.phase != Phase::Candidate {
            return Ok(());
        }
        let candidate = self
            .candidate
            .as_ref()
            .context("actual unselected candidate")?;
        if let Some(current) = named(&self.namespace, candidate.name.as_str())? {
            anyhow::ensure!(
                identity(&current)? == identity(&candidate.file)?,
                "private selector candidate replaced; actual owner retained"
            );
            rustix::fs::unlinkat(
                &self.namespace,
                candidate.name.as_str(),
                rustix::fs::AtFlags::empty(),
            )?;
            self.namespace.sync_all()?;
        }
        self.candidate.take();
        self.phase = Phase::Unselected;
        Ok(())
    }
    pub(super) fn retirement_context(&self, owner: &NativeCorpusDraft) -> Result<(File, String)> {
        owner.check()?;
        self.check(owner)?;
        anyhow::ensure!(
            self.writer_locked && self.phase == Phase::Durable,
            "retirement requires the actual durable selector writer"
        );
        self.check_selected()?;
        Ok((
            self.namespace.try_clone()?,
            self.candidate
                .as_ref()
                .context("actual selected candidate")?
                .pointer
                .stage
                .clone(),
        ))
    }
    #[cfg(test)]
    pub fn selected_pointer(&self) -> Result<&Pointer> {
        self.check_selected()?;
        Ok(&self
            .candidate
            .as_ref()
            .context("actual selected tuple")?
            .pointer)
    }
}

/// The published native generation can be searched as-is.
/// Any doubt, including a newer page or wiki mtime, returns false so startup rebuilds.
impl NativeLibraryService {
    pub fn published_corpus_current(&self, permit: &Permit) -> bool {
        self.published_corpus_current_inner(permit).unwrap_or(false)
    }
    fn published_corpus_current_inner(&self, permit: &Permit) -> Result<bool> {
        let root = permit
            .with_selected_context(&self.library, |root, _, _| root.try_clone().map_err(refused))
            .map_err(refused_error)?;
        let root_meta = root.metadata()?;
        let original_root = (root_meta.dev(), root_meta.ino());
        let parent = Account::current()?.generated_directory()?;
        let namespace_name = namespace_name(self.library.as_str(), original_root);
        let namespace = match open_directory(&parent, &namespace_name) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let Some(pointer_file) = named(&namespace, "CURRENT")? else {
            return Ok(false);
        };
        let pointer_bytes = read_capped(&pointer_file, 1024 * 1024)?;
        let pointer: Pointer = serde_json::from_slice(&pointer_bytes)?;
        if pointer.format != 1
            || pointer.library != self.library.as_str()
            || pointer.original_root != original_root
            || !pointer.stage.starts_with("native-corpus-")
            || pointer.stage.len() > 255
            || pointer.stage.contains('/')
            || pointer.stage.contains('\\')
            || std::path::Path::new(&pointer.stage).components().count() != 1
        {
            return Ok(false);
        }
        let stage = match open_directory(&namespace, &pointer.stage) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let stage_meta = stage.metadata()?;
        if (stage_meta.dev(), stage_meta.ino()) != pointer.stage_root {
            return Ok(false);
        }
        let Some(header_file) = named(&stage, "tuple.json")? else {
            return Ok(false);
        };
        let header_bytes = read_capped(&header_file, 1024 * 1024)?;
        if hex::encode(Sha256::digest(&header_bytes)) != pointer.header_sha256 {
            return Ok(false);
        }
        let header: tuple::Header = serde_json::from_slice(&header_bytes)?;
        if header.format != 1
            || header.library != self.library.as_str()
            || header.original_root != original_root
            || header.stage_root != pointer.stage_root
            || header.machinery != crate::admitted::build_identity()
        {
            return Ok(false);
        }
        let journal = match open_directory(&root, "journal") {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let Some((scan, observed_at)) = latest_complete_checkpoint(&journal)? else {
            return Ok(false);
        };
        if header.scan != scan {
            return Ok(false);
        }
        let observed = time::OffsetDateTime::parse(
            &observed_at,
            &time::format_description::well_known::Rfc3339,
        )?;
        let observed = offset_to_system(observed)?;
        let pages = directory_freshness(&root, "pages", observed)?;
        let wiki = directory_freshness(&root, "wiki", observed)?;
        Ok(match (pages, wiki) {
            (Freshness::Missing, Freshness::Missing) => false,
            (Freshness::Newer, _) | (_, Freshness::Newer) => false,
            _ => true,
        })
    }
}
enum Freshness {
    Missing,
    Current,
    Newer,
}
fn open_directory(parent: &File, name: &str) -> std::io::Result<File> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}
fn read_capped(file: &File, limit: u64) -> Result<Vec<u8>> {
    use std::os::unix::fs::FileExt;
    let len = file.metadata()?.len();
    anyhow::ensure!(len <= limit, "published corpus metadata exceeds the cap");
    let mut bytes = vec![0; usize::try_from(len)?];
    file.read_exact_at(&mut bytes, 0)?;
    Ok(bytes)
}
fn latest_complete_checkpoint(journal: &File) -> Result<Option<(String, String)>> {
    use std::io::{BufRead, BufReader};
    use std::os::unix::io::AsRawFd;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", journal.as_raw_fd()))? {
        let name = entry?.file_name();
        let text = name.to_string_lossy();
        if text.ends_with(".jsonl") && !text.contains('/') {
            names.push(name);
        }
    }
    names.sort();
    let mut found = None;
    for name in names {
        let file = match named(journal, &name.to_string_lossy())? {
            Some(file) => file,
            None => continue,
        };
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)?;
            let Some(checkpoint) = value.get("checkpoint") else {
                continue;
            };
            if checkpoint.get("complete").and_then(|item| item.as_bool()) != Some(true) {
                continue;
            }
            let Some(scan) = checkpoint.get("scan").and_then(|item| item.as_str()) else {
                continue;
            };
            let Some(observed_at) = checkpoint
                .get("observed_at")
                .and_then(|item| item.as_str())
            else {
                continue;
            };
            if !scan.is_empty() && !observed_at.is_empty() {
                found = Some((scan.to_owned(), observed_at.to_owned()));
            }
        }
    }
    Ok(found)
}
fn offset_to_system(observed: time::OffsetDateTime) -> Result<std::time::SystemTime> {
    let unix = observed.unix_timestamp();
    anyhow::ensure!(unix >= 0, "checkpoint time is not a unix timestamp");
    Ok(std::time::UNIX_EPOCH + std::time::Duration::new(unix as u64, observed.nanosecond()))
}
fn directory_freshness(
    root: &File,
    name: &str,
    observed: std::time::SystemTime,
) -> Result<Freshness> {
    use std::os::unix::io::AsRawFd;
    // Stat the child itself. /proc/self/fd/<root> is only an intermediate link, so
    // symlink_metadata does not follow `name` and does not stat the fd symlink.
    let meta = match std::fs::symlink_metadata(format!(
        "/proc/self/fd/{}/{}",
        root.as_raw_fd(),
        name
    )) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Freshness::Missing),
        Err(error) => return Err(error.into()),
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Ok(if meta.modified()? > observed {
            Freshness::Newer
        } else {
            Freshness::Current
        });
    }
    if meta.modified()? > observed {
        return Ok(Freshness::Newer);
    }
    let dir = open_directory(root, name)?;
    if tree_newer(&dir, observed)? {
        Ok(Freshness::Newer)
    } else {
        Ok(Freshness::Current)
    }
}
fn tree_newer(dir: &File, observed: std::time::SystemTime) -> Result<bool> {
    use std::os::unix::io::AsRawFd;
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))? {
        let entry = entry?;
        let name = entry.file_name();
        let mut path = std::path::PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()));
        path.push(&name);
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.modified()? > observed {
            return Ok(true);
        }
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        let child = open_directory(dir, &name.to_string_lossy())?;
        if tree_newer(&child, observed)? {
            return Ok(true);
        }
    }
    Ok(false)
}
