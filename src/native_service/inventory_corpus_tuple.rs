//! One bounded immutable tuple seal around the existing engines. No independent component selection.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Member {
    pub name: String,
    pub directory: bool,
    pub version: [u64; 8],
    pub sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub format: u32,
    pub library: String,
    pub original_root: (u64, u64),
    pub stage_root: (u64, u64),
    pub scan: String,
    pub descriptor: String,
    pub recipe: String,
    pub processor: String,
    pub machinery: String,
    pub model: Option<knowledge_search::ModelIdentity>,
    pub vector_generation: Option<String>,
    pub sources: u64,
    pub shards: u64,
    pub planned_embedding_rows: u64,
    pub vector_rows: u64,
    pub coverage: NativeCorpusCoverage,
    pub members: u64,
    pub members_length: u64,
    pub members_sha256: String,
}
// Always nested inside the actual draft until an actual publication/query owner accepts it.
// Buffers/FDs/pins above die before the draft's existing paid metadata Work.
pub(super) struct Seal {
    pub header: File,
    pub members: File,
    pub readers: File,
    pub header_data: Header,
    pub header_version: [u64; 8],
    pub members_version: [u64; 8],
    pub readers_version: [u64; 8],
}
impl Seal {
    pub fn verify_members(
        &self,
        files: &Files,
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        self.check(files)?;
        let mut offset = 0;
        let mut count = 0u64;
        while offset < self.members_version[2] {
            check()?;
            let (member, next) = read_frame::<Member>(&self.members, offset)?;
            let file = open_member(&files.directory, &member.name)?;
            let metadata = file.metadata()?;
            anyhow::ensure!(
                metadata.is_dir() == member.directory && stamp(&file)? == member.version,
                "sealed tuple component identity/version changed"
            );
            check()?;
            count += 1;
            offset = next;
        }
        anyhow::ensure!(
            count == self.header_data.members,
            "incomplete tuple member coverage"
        );
        self.check(files)?;
        check()
    }
    pub fn check(&self, files: &Files) -> Result<()> {
        anyhow::ensure!(
            self.header_data.format == 1
                && self.header_data.members_length == self.members_version[2],
            "inconsistent tuple header facts"
        );
        anyhow::ensure!(
            stamp(&self.header)? == self.header_version
                && stamp(&self.members)? == self.members_version
                && stamp(&self.readers)? == self.readers_version,
            "actual tuple descriptors changed"
        );
        for (name, file) in [
            ("tuple.json", &self.header),
            ("members.rows", &self.members),
            ("READERS", &self.readers),
        ] {
            let named = rustix::fs::statat(
                &files.directory,
                name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )?;
            let held = file.metadata()?;
            anyhow::ensure!(
                (named.st_dev as _, named.st_ino as _) == (held.dev(), held.ino()),
                "tuple member identity changed"
            );
        }
        Ok(())
    }
}
pub(super) fn open_member(directory: &File, name: &str) -> Result<File> {
    let parts = Path::new(name).components();
    anyhow::ensure!(
        !name.is_empty() && parts.clone().count() <= 33,
        "invalid native component hierarchy"
    );
    let mut parts = parts.peekable();
    let mut current = directory.try_clone()?;
    while let Some(part) = parts.next() {
        let std::path::Component::Normal(name) = part else {
            bail!("invalid native component path")
        };
        let mut flags = rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK;
        if parts.peek().is_some() {
            flags |= rustix::fs::OFlags::DIRECTORY;
        }
        current = File::from(rustix::fs::openat(
            &current,
            name,
            flags,
            rustix::fs::Mode::empty(),
        )?);
    }
    Ok(current)
}
fn create(directory: &File, name: &str) -> Result<File> {
    Ok(File::from(rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_bits_truncate(0o600),
    )?))
}
pub(super) fn hash(file: &File, mut check: impl FnMut() -> Result<()>) -> Result<String> {
    let before = stamp(file)?;
    let mut digest = Sha256::new();
    let mut offset = 0u64;
    let mut bytes = vec![0; READ_WINDOW];
    while offset < before[2] {
        check()?;
        let n = (before[2] - offset).min(bytes.len() as u64) as usize;
        file.read_exact_at(&mut bytes[..n], offset)?;
        digest.update(&bytes[..n]);
        offset += n as u64;
        check()?;
    }
    anyhow::ensure!(
        stamp(file)? == before,
        "component changed during tuple sealing"
    );
    Ok(hex::encode(digest.finalize()))
}
fn append(
    owner: &mut NativeCorpusDraft,
    file: &mut File,
    member: &Member,
    progress: &mut impl FnMut(NativeCorpusProgress),
) -> Result<()> {
    let bytes = frame_bytes(member)?;
    owner.wait_growth(bytes.len() as u64, None, progress)?;
    let before = file.metadata()?.len();
    let result = file.write_all(&bytes);
    let after = file.metadata()?.len();
    owner
        .work
        .as_mut()
        .context("actual tuple metadata loan")?
        .written(
            after
                .checked_sub(before)
                .context("tuple metadata regressed")?,
        )?;
    result?;
    Ok(())
}
fn walk(
    owner: &mut NativeCorpusDraft,
    directory: File,
    prefix: &str,
    depth: usize,
    members: &mut File,
    count: &mut u64,
    progress: &mut impl FnMut(NativeCorpusProgress),
) -> Result<()> {
    // The native generated format has fixed-depth kernel/vector components, not arbitrary original paths.
    anyhow::ensure!(depth <= 32, "unexpected native component hierarchy");
    let before = stamp(&directory)?;
    for entry in rustix::fs::Dir::read_from(&directory)? {
        owner.check()?;
        let entry = entry?;
        let raw = entry.file_name().to_bytes();
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(raw).context("native component name is not UTF-8")?;
        if prefix.is_empty() && matches!(name, "tuple.json" | "members.rows" | "READERS") {
            continue;
        }
        anyhow::ensure!(
            !name.is_empty() && !name.contains('/') && !name.contains('\\'),
            "invalid native component name"
        );
        let child = File::from(rustix::fs::openat(
            &directory,
            entry.file_name(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = child.metadata()?;
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        anyhow::ensure!(
            metadata.is_dir() || metadata.is_file(),
            "unexpected nonregular generated component"
        );
        let sha = if metadata.is_file() {
            Some(hash(&child, || owner.check())?)
        } else {
            None
        };
        let member = Member {
            name: path.clone(),
            directory: metadata.is_dir(),
            version: stamp(&child)?,
            sha256: sha,
        };
        append(owner, members, &member, progress)?;
        *count = count
            .checked_add(1)
            .context("tuple member count overflow")?;
        if metadata.is_dir() {
            walk(owner, child, &path, depth + 1, members, count, progress)?;
        }
    }
    anyhow::ensure!(
        stamp(&directory)? == before,
        "native directory changed during tuple sealing"
    );
    Ok(())
}
pub(super) fn prepare(
    owner: &mut NativeCorpusDraft,
    progress: &mut impl FnMut(NativeCorpusProgress),
) -> Result<Seal> {
    owner.check()?;
    anyhow::ensure!(owner.lexical_done, "tuple lacks complete lexical phase");
    anyhow::ensure!(
        owner.coverage.observations
            == owner
                .sources
                .checked_add(owner.coverage.unavailable)
                .and_then(|n| n.checked_add(owner.coverage.tombstones))
                .context("tuple source coverage overflow")?,
        "tuple source coverage is incomplete"
    );
    let model = owner
        .job
        .cause
        .model
        .as_ref()
        .map(|m| m.model().cloned())
        .transpose()?;
    anyhow::ensure!(
        if model.is_some() {
            owner.vectors_done
                && ((owner.rows == 0 && owner.vectors.is_none())
                    || (owner.rows > 0 && owner.vectors.is_some()))
        } else {
            owner.vectors.is_none()
        },
        "tuple lacks the selected model/vector phase"
    );
    if let Some(vectors) = &owner.vectors {
        anyhow::ensure!(
            Arc::strong_count(vectors) == 1,
            "actual vector component consumer still owns preparation"
        );
        vectors.check()?;
    }
    let directory = owner
        .files
        .as_ref()
        .context("actual tuple directory")?
        .directory
        .try_clone()?;
    // These actual created descriptors never get replaced by a hash-then-reopen consume path.
    let readers = create(&directory, "READERS")?;
    fs2::FileExt::lock_shared(&readers)?;
    let mut header = create(&directory, "tuple.json")?;
    let mut members = create(&directory, "members.rows")?;
    let mut count = 0;
    walk(
        owner,
        directory.try_clone()?,
        "",
        0,
        &mut members,
        &mut count,
        progress,
    )?;
    members.sync_all()?;
    let members_sha256 = hash(&members, || owner.check())?;
    let m = directory.metadata()?;
    let data = Header {
        format: 1,
        library: owner.job.cause.library.as_str().to_owned(),
        original_root: owner.job.cause.inventory.owner.root,
        stage_root: (m.dev(), m.ino()),
        scan: owner.job.cause.inventory.owner.scan.clone(),
        descriptor: owner.job.cause.version.as_str().to_owned(),
        recipe: owner.job.cause.recipe.as_str().to_owned(),
        processor: owner.job.cause.processor.as_str().to_owned(),
        machinery: admitted::build_identity(),
        model,
        vector_generation: owner.vectors.as_ref().map(|v| v.generation().to_owned()),
        sources: owner.sources,
        shards: owner.shards,
        planned_embedding_rows: owner.rows,
        vector_rows: if owner.vectors.is_some() {
            owner.rows
        } else {
            0
        },
        coverage: owner.coverage,
        members: count,
        members_length: members.metadata()?.len(),
        members_sha256,
    };
    let bytes = serde_json::to_vec(&data)?;
    anyhow::ensure!(
        bytes.len() <= FRAME,
        "native tuple header exceeds existing frame format"
    );
    owner.wait_growth(bytes.len() as u64, None, progress)?;
    let before = header.metadata()?.len();
    let result = header.write_all(&bytes);
    let after = header.metadata()?.len();
    owner
        .work
        .as_mut()
        .context("actual tuple metadata loan")?
        .written(
            after
                .checked_sub(before)
                .context("tuple header regressed")?,
        )?;
    result?;
    header.sync_all()?;
    readers.sync_all()?;
    directory.sync_all()?;
    owner.check()?;
    let seal = Seal {
        header_version: stamp(&header)?,
        members_version: stamp(&members)?,
        readers_version: stamp(&readers)?,
        header,
        members,
        readers,
        header_data: data,
    };
    seal.verify_members(owner.files.as_ref().context("actual tuple files")?, || {
        owner.check()
    })?;
    Ok(seal)
}
