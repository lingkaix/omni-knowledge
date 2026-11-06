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
