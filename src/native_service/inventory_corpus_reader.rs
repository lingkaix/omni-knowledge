//! Actual readonly whole-tuple opening: aggregate pin precedes every header/component open.
use super::*;
use omni_service::authority::SelectionLease;
use sha2::{Digest, Sha256};

const READER_MEMORY: u64 = 8 * 1024 * 1024;
#[cfg(test)]
thread_local! {static TEST_UNPINNED_HEADERS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
struct Inputs {
    permit: Arc<Permit>,
    funding: Option<Work>,
}
/// An actual Query/root/aggregate-pin owner, not a path/hash/manifest reconstruction grant.
/// Controllers keep this owner through all component consumers and bounded delivery.
pub struct NativeTupleRead {
    pub(super) physical: Arc<Physical>,
    pub(super) permit: Arc<Permit>,
    pub(super) deadline: Instant,
}
pub(super) struct Physical {
    parent: File,
    namespace: File,
    namespace_name: String,
    pub stage: File,
    pub readers: File,
    pub header: File,
    pub members: File,
    source: File,
    pub(super) library: wire::Id,
    lease: SelectionLease,
    pub pointer: selection::Pointer,
    pub data: tuple::Header,
    header_version: [u64; 8],
    members_version: [u64; 8],
    readers_version: [u64; 8],
    // Actual descriptors/pins and all metadata above die before their one paid resource lifetime.
    _funding: Work,
}
fn open(directory: &File, name: &str, directory_only: bool) -> Result<File> {
    #[cfg(test)]
    if name == "tuple.json" && !directory_only {
        let probe = open(directory, "READERS", false)?;
        if fs2::FileExt::try_lock_exclusive(&probe).is_ok() {
            TEST_UNPINNED_HEADERS.with(|count| count.set(count.get() + 1));
        }
    }
    let mut flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::NONBLOCK;
    if directory_only {
        flags |= rustix::fs::OFlags::DIRECTORY;
    }
    Ok(File::from(rustix::fs::openat(
        directory,
        name,
        flags,
        rustix::fs::Mode::empty(),
    )?))
}
fn same(a: &File, b: &File) -> Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok((a.dev(), a.ino()) == (b.dev(), b.ino()))
}
fn read_small(file: &File, mut check: impl FnMut() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    let before = stamp(file)?;
    anyhow::ensure!(
        before[2] <= FRAME as u64,
        "tuple metadata frame exceeds existing format"
    );
    let mut bytes = vec![0; usize::try_from(before[2])?];
    file.read_exact_at(&mut bytes, 0)?;
    check()?;
    anyhow::ensure!(stamp(file)? == before, "tuple metadata changed while read");
    Ok(bytes)
}
impl Physical {
    pub(super) fn member_exists(
        &self,
        name: &str,
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        self.check()?;
        let mut offset = 0;
        while offset < self.members_version[2] {
            check()?;
            let (member, next) = read_frame::<tuple::Member>(&self.members, offset)?;
            offset = next;
            if member.name == name {
                self.check()?;
                check()?;
                return Ok(!member.directory);
            }
        }
        self.check()?;
        Ok(false)
    }
    pub(super) fn open_verified(
        &self,
        name: &str,
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<File> {
        self.check()?;
        let mut offset = 0;
        while offset < self.members_version[2] {
            check()?;
            let (member, next) = read_frame::<tuple::Member>(&self.members, offset)?;
            offset = next;
            if member.name == name {
                anyhow::ensure!(!member.directory, "tuple component is not a regular file");
                let file = tuple::open_member(&self.stage, name)?;
                anyhow::ensure!(
                    file.metadata()?.is_file() && stamp(&file)? == member.version,
                    "tuple component identity/version changed"
                );
                check()?;
                self.check()?;
                return Ok(file);
            }
        }
        bail!("required component is absent from complete tuple")
    }
    pub fn check(&self) -> Result<()> {
        self.lease.check().map_err(refused_error)?;
        anyhow::ensure!(
            same(&Account::current()?.generated_directory()?, &self.parent)?,
            "tuple generated owner changed"
        );
        anyhow::ensure!(
            same(
                &open(&self.parent, &self.namespace_name, true)?,
                &self.namespace
            )?,
            "tuple namespace identity changed"
        );
        anyhow::ensure!(
            same(
                &open(&self.namespace, &self.pointer.stage, true)?,
                &self.stage
            )?,
            "pinned tuple generation identity changed"
        );
        anyhow::ensure!(
            stamp(&self.header)? == self.header_version
                && stamp(&self.members)? == self.members_version
                && stamp(&self.readers)? == self.readers_version,
            "pinned tuple metadata version changed"
        );
        for (name, file) in [
            ("tuple.json", &self.header),
            ("members.rows", &self.members),
            ("READERS", &self.readers),
        ] {
            anyhow::ensure!(
                same(&open(&self.stage, name, false)?, file)?,
                "pinned tuple metadata identity changed"
            );
        }
        Ok(())
    }
    fn verify_all(&self, mut check: impl FnMut() -> Result<()>) -> Result<()> {
        let mut offset = 0;
        let mut count = 0;
        while offset < self.members_version[2] {
            check()?;
            let (member, next) = read_frame::<tuple::Member>(&self.members, offset)?;
            let file = tuple::open_member(&self.stage, &member.name)?;
            let metadata = file.metadata()?;
            anyhow::ensure!(
                metadata.is_dir() == member.directory && stamp(&file)? == member.version,
                "incomplete or changed whole tuple component"
            );
            offset = next;
            count += 1;
        }
        anyhow::ensure!(
            count == self.data.members,
            "whole tuple member coverage differs"
        );
        self.check()?;
        check()
    }
}
pub(super) struct TuplePart {
    data: SourcePart,
    physical: Arc<Physical>,
    source_rows: Arc<File>,
    rows_version: [u64; 8],
}
impl TuplePart {
    fn check(&self, file: &File) -> Result<()> {
        self.physical.check()?;
        anyhow::ensure!(
            stamp(&self.source_rows)? == self.rows_version
                && stamp(file)? == self.data.original_version,
            "cached source/component dependency changed"
        );
        Ok(())
    }
    pub(super) fn source(
        self,
        service: &NativeLibraryService,
        permit: &Permit,
        observed: NativeObservedSource,
    ) -> Result<NativeSource> {
        self.physical
            .lease
            .check_permit(permit)
            .map_err(refused_error)?;
        anyhow::ensure!(
            service.library == self.physical.library
                && self.data.document.key.namespace == service.library.as_str()
                && self.data.document.key.id == observed.source_id()
                && self.data.document.locator.as_deref()
                    == Some(observed.observation().stat.path.as_str()),
            "opaque tuple part does not select this actual M1 source"
        );
        let digest = knowledge_core::data::ContentDigest::new(self.data.document.digest.clone())?;
        let version = self.data.original_version;
        NativeSource::open_cached_query(
            &service.library,
            permit,
            observed,
            version,
            digest,
            Arc::new(move |file| self.check(file)),
        )
    }
}
impl NativeTupleRead {
    /// Bind one opaque cached part to a SEPARATE actual current M1 row and Query read scope.
    /// Checks actual original FD/stamp without whole-NAS body hashing; stale requires Refresh.
    /// Returned source retains this aggregate metadata/pin owner through its last read/delivery.
    pub fn current_source_at(
        &self,
        service: &NativeLibraryService,
        permit: &Permit,
        observed: NativeObservedSource,
        offset: u64,
    ) -> Result<Option<(NativeSource, u64)>> {
        match self.part(offset)? {
            Some((part, next)) => Ok(Some((part.source(service, permit, observed)?, next))),
            None => Ok(None),
        }
    }
    pub(super) fn checked_part_at(
        &self,
        service: &NativeLibraryService,
        permit: &Permit,
        observed: NativeObservedSource,
        offset: u64,
    ) -> Result<Option<(NativeSource, SourcePart, u64)>> {
        let Some((part, next)) = self.part(offset)? else {
            return Ok(None);
        };
        let data = part.data.clone();
        let source = part.source(service, permit, observed)?;
        Ok(Some((source, data, next)))
    }
    fn part(&self, offset: u64) -> Result<Option<(TuplePart, u64)>> {
        self.check()?;
        let file = Arc::new(
            self.physical
                .open_verified("sources.rows", || self.check())?,
        );
        let version = stamp(&file)?;
        if offset == version[2] {
            return Ok(None);
        }
        anyhow::ensure!(
            offset < version[2],
            "source cursor is outside selected tuple"
        );
        let (data, next) = read_frame::<SourcePart>(&file, offset)?;
        self.check()?;
        anyhow::ensure!(
            stamp(&file)? == version,
            "source table changed while reading part"
        );
        Ok(Some((
            TuplePart {
                data,
                physical: self.physical.clone(),
                source_rows: file,
                rows_version: version,
            },
            next,
        )))
    }
    /// Complete bounded metadata/root/header/manifest/pin envelope, not kernel/vector query costs.
    pub fn requirement() -> Reservation {
        Reservation {
            memory_bytes: READER_MEMORY,
            temporary_bytes: 0,
        }
    }
    /// Recheck actual Query, selected root, policy, deadline and all pinned metadata identities.
    pub fn check(&self) -> Result<()> {
        anyhow::ensure!(
            Instant::now() < self.deadline,
            "tuple query deadline elapsed"
        );
        self.permit.check().map_err(refused_error)?;
        self.physical.check()?;
        let actual = self
            .permit
            .with_selected_context(&self.physical.library, |root, _, _| {
                root.metadata().map_err(refused)
            })
            .map_err(refused_error)?;
        let held = self.physical.source.metadata()?;
        anyhow::ensure!(
            (actual.dev(), actual.ino()) == (held.dev(), held.ino()),
            "actual Query selected root changed"
        );
        Ok(())
    }
    /// Immutable whole-tuple identity data. It grants no source, model or component permission.
    pub fn generation(&self) -> Result<&str> {
        self.check()?;
        Ok(&self.physical.pointer.stage)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_unpinned_header_opens(&self) -> usize {
        TEST_UNPINNED_HEADERS.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_part_path(&self, offset: u64) -> Result<String> {
        self.part(offset)?
            .context("actual cached source part")?
            .0
            .data
            .document
            .locator
            .context("actual source display metadata")
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_source_partition(
        &self,
        path: &str,
    ) -> Result<(u64, u64)> {
        let mut at = 0;
        let (source_at, data) = loop {
            let Some((part, next)) = self.part(at)? else {
                bail!("actual fixture source receipt absent")
            };
            if part.data.document.locator.as_deref() == Some(path) {
                break (at, part.data);
            }
            at = next;
        };
        let file = self
            .physical
            .open_verified("lexical.rows", || self.check())?;
        let mut at = 0;
        while at < file.metadata()?.len() {
            let (shard, next) = read_frame::<Shard>(&file, at)?;
            if shard.parts.contains(&data) {
                return Ok((source_at, at));
            }
            at = next;
        }
        bail!("actual fixture source partition absent")
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_header(&self) -> Result<serde_json::Value> {
        self.check()?;
        Ok(serde_json::to_value(&self.physical.data)?)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_aggregate_exclusive(&self) -> Result<bool> {
        self.check()?;
        let file = open(&self.physical.stage, "READERS", false)?;
        Ok(fs2::FileExt::try_lock_exclusive(&file).is_ok())
    }
}
impl NativeLibraryService {
    /// Open ONLY an existing selected tuple under the actual Query/Owner and Core volume scope.
    /// No reconciliation, mkdir, journal upgrade, independent component CURRENT or payload query.
    /// The returned actual aggregate owner must survive all later consumers/delivery.
    pub fn open_corpus_tuple(
        &self,
        owner: &Owner,
        permit: Permit,
        funding: Work,
    ) -> Result<NativeTupleRead> {
        anyhow::ensure!(
            funding.remaining()?.memory_bytes >= READER_MEMORY,
            "complete tuple reader metadata envelope is not prepaid"
        );
        let mut inputs = Inputs {
            permit: Arc::new(permit),
            funding: Some(funding),
        };
        let wire::Command::Query { query } =
            inputs.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Query required for tuple read")
        };
        anyhow::ensure!(
            query.library_ids.contains(&self.library)
                && query.budget.max_memory_bytes.get() >= READER_MEMORY,
            "tuple Query selection/budget differs"
        );
        let max_bytes = query.budget.max_bytes.get();
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(query.budget.max_millis.get()))
            .context("tuple query deadline overflow")?;
        let lease = owner
            .pin_selection(&self.selection)
            .map_err(refused_error)?;
        lease.check_permit(&inputs.permit).map_err(refused_error)?;
        let source = inputs
            .permit
            .with_selected_context(&self.library, |root, _, _| {
                root.try_clone().map_err(refused)
            })
            .map_err(refused_error)?;
        let root = source.metadata()?;
        let original_root = (root.dev(), root.ino());
        let parent = Account::current()?.generated_directory()?;
        let namespace_name = selection::namespace_name(self.library.as_str(), original_root);
        let namespace = open(&parent, &namespace_name, true)?;
        let writer = open(&namespace, "WRITER", false)?;
        let check = || -> Result<()> {
            anyhow::ensure!(Instant::now() < deadline, "tuple query deadline elapsed");
            inputs.permit.check().map_err(refused_error)?;
            lease.check().map_err(refused_error)?;
            Ok(())
        };
        loop {
            check()?;
            match fs2::FileExt::try_lock_shared(&writer) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(e.into()),
            }
        }
        let pointer_file = open(&namespace, "CURRENT", false)?;
        let pointer_version = stamp(&pointer_file)?;
        let bytes = read_small(&pointer_file, &check)?;
        let mut spent = bytes.len() as u64;
        let pointer: selection::Pointer = serde_json::from_slice(&bytes)?;
        drop(bytes);
        anyhow::ensure!(
            pointer.format == 1
                && pointer.library == self.library.as_str()
                && pointer.original_root == original_root,
            "foreign whole tuple selector"
        );
        anyhow::ensure!(
            pointer.stage.starts_with("native-corpus-")
                && pointer.stage.len() <= 255
                && !pointer.stage.contains('/')
                && !pointer.stage.contains('\\')
                && Path::new(&pointer.stage).components().count() == 1,
            "invalid whole tuple generation name"
        );
        let stage = open(&namespace, &pointer.stage, true)?;
        let m = stage.metadata()?;
        anyhow::ensure!(
            (m.dev(), m.ino()) == pointer.stage_root,
            "selected generation identity differs"
        );
        // ACTUAL aggregate ownership is acquired before opening header or any component.
        let readers = open(&stage, "READERS", false)?;
        loop {
            check()?;
            match fs2::FileExt::try_lock_shared(&readers) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(e.into()),
            }
        }
        let readers_version = stamp(&readers)?;
        let header = open(&stage, "tuple.json", false)?;
        anyhow::ensure!(
            stamp(&header)? == pointer.header_version,
            "selected tuple header identity/version differs"
        );
        let bytes = read_small(&header, &check)?;
        spent = spent
            .checked_add(bytes.len() as u64)
            .context("tuple read bytes overflow")?;
        anyhow::ensure!(
            hex::encode(Sha256::digest(&bytes)) == pointer.header_sha256,
            "whole tuple header checksum differs"
        );
        let data: tuple::Header = serde_json::from_slice(&bytes)?;
        drop(bytes);
        anyhow::ensure!(
            data.format == 1
                && data.library == self.library.as_str()
                && data.original_root == original_root
                && data.stage_root == pointer.stage_root
                && data.machinery == admitted::build_identity(),
            "whole tuple consuming/root identity differs; rebuild required"
        );
        anyhow::ensure!(
            data.coverage.observations
                == data
                    .sources
                    .checked_add(data.coverage.unavailable)
                    .and_then(|n| n.checked_add(data.coverage.tombstones))
                    .context("tuple coverage overflow")?,
            "whole tuple source coverage is incomplete"
        );
        let members = open(&stage, "members.rows", false)?;
        let members_version = stamp(&members)?;
        anyhow::ensure!(
            members_version[2] == data.members_length,
            "whole tuple member length differs"
        );
        spent = spent
            .checked_add(data.members_length)
            .and_then(|n| n.checked_add(data.members_length))
            .context("tuple metadata read bound overflow")?;
        anyhow::ensure!(
            spent <= max_bytes,
            "actual Query metadata byte budget exhausted"
        );
        anyhow::ensure!(
            tuple::hash(&members, &check)? == data.members_sha256,
            "whole tuple member manifest checksum differs"
        );
        check()?;
        anyhow::ensure!(
            stamp(&pointer_file)? == pointer_version
                && same(&open(&namespace, "CURRENT", false)?, &pointer_file)?,
            "whole tuple changed while aggregate pin was acquired"
        );
        let physical = Arc::new(Physical {
            parent,
            namespace,
            namespace_name,
            stage,
            readers,
            header_version: stamp(&header)?,
            header,
            members,
            members_version,
            source,
            library: self.library.clone(),
            lease,
            pointer,
            data,
            readers_version,
            _funding: inputs.funding.take().expect("actual paid reader lifetime"),
        });
        let reader = NativeTupleRead {
            physical,
            permit: inputs.permit,
            deadline,
        };
        reader.physical.verify_all(|| reader.check())?;
        reader.check()?;
        drop(pointer_file);
        drop(writer); // Actual aggregate pin now protects unopened components.
        Ok(reader)
    }
}
