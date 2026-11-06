//! The existing M5 verifier/range engine, without requiring a resident lexical index.
use super::{AdmittedRange, ArchiveDirectory, ArchiveSource, Backing, StoredIdentity, Version};
use crate::{Control, Error, Work};
use knowledge_package::{Bounds, FrozenArchive, IndexRequirements, Method, Role};
use retrieval_kernel::ByteDirectory;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

/// Facts for one complete simultaneous read-only envelope, not spending credit.
#[derive(Clone, Copy, Debug)]
pub struct StoredDirectoryPlan {
    /// Declared maximum total index payload bytes. This does NOT become resident memory.
    pub index_bytes: u64,
    /// Maximum complete retained range/name table, including construction copies.
    pub table_bytes: u64,
    /// Verifier's bounded portion of the actual complete holding.
    pub verification_memory: u64,
    /// Largest permitted single range allocation; consumers separately pay their own buffers.
    pub read_window_bytes: u64,
}
impl StoredDirectoryPlan {
    /// Reserve before source inspection. There is no corpus-byte ceiling or whole-payload buffer.
    pub fn requirement(self) -> Result<knowledge_core::jobs::Reservation, Error> {
        if self.verification_memory == 0 || self.read_window_bytes == 0 {
            return Err(Error::Reservation);
        }
        usize::try_from(self.read_window_bytes).map_err(|_| Error::Inventory)?;
        let memory_bytes = self
            .table_bytes
            .checked_mul(2)
            .and_then(|n| {
                self.read_window_bytes
                    .checked_mul(4)
                    .and_then(|w| n.checked_add(w))
            })
            .and_then(|n| n.checked_add(self.verification_memory))
            .and_then(|n| n.checked_add(64 * 1024))
            .ok_or(Error::Inventory)?;
        Ok(knowledge_core::jobs::Reservation {
            memory_bytes,
            temporary_bytes: 0,
        })
    }
}

/// Actual verified STORED components over the original held archive and native currentness owner.
/// It shares the existing range engine, never extracts data or opens another index engine.
/// The complete caller-supplied model-space facts are checked by M5 alongside the native formats;
/// the host still binds its actual selected model/source/Claim and current policy, not DTO flags.
/// Each consuming reader must retain its own prepaid query envelope as well as this shared owner.
pub struct StoredDirectory {
    backing: Arc<Backing>,
}
impl StoredDirectory {
    /// Verify with the actual source and same-account Work before publishing any range handle.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        source: Arc<dyn ArchiveSource>,
        native: &IndexRequirements<'_>,
        build_identity: &str,
        plan: StoredDirectoryPlan,
        mut work: Work,
        bounds: &Bounds,
        cancel: &AtomicBool,
        control: &Control,
    ) -> Result<Self, Error> {
        let need = plan.requirement()?;
        work.require(need.memory_bytes, 0)?;
        let actual = StoredIdentity::new(build_identity)?;
        let expected = actual.requirements(native.generation);
        if native.kernel_build != expected.kernel_build
            || native.lexical_format != expected.lexical_format
            || native.segment_format != expected.segment_format
            || native.processor != expected.processor
            || native.dictionary != expected.dictionary
        {
            return Err(Error::Inventory);
        }
        source.check()?;
        control.check()?;
        let file = source.file()?.try_clone().map_err(|_| Error::Inventory)?;
        let version = Version::of(&file.metadata().map_err(|_| Error::Inventory)?);
        let current = || {
            control
                .check()
                .map_err(|_| knowledge_package::Error::Cancelled)?;
            source.check().map_err(|_| knowledge_package::Error::Drift)
        };
        let mut package_work = knowledge_package::Work::guarded_reader_capped(
            bounds,
            &mut work.0,
            cancel,
            &current,
            plan.verification_memory,
        )?;
        let mut frozen = FrozenArchive::open(file, &mut package_work)?;
        frozen.index_paths(native)?;
        let mut index_bytes = 0u64;
        let mut table_bytes = 0u64;
        for entry in &frozen.manifest().entries {
            control.check()?;
            source.check()?;
            if entry.role == Role::Index {
                index_bytes = index_bytes
                    .checked_add(entry.size_bytes)
                    .ok_or(Error::Inventory)?;
            }
            // Include every manifest path; the verifier has a separate capped lifetime.
            table_bytes = table_bytes
                .checked_add(
                    (entry.path.as_str().len() as u64)
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(512))
                        .ok_or(Error::Inventory)?,
                )
                .ok_or(Error::Inventory)?;
        }
        if index_bytes > plan.index_bytes || table_bytes > plan.table_bytes {
            return Err(Error::Reservation);
        }
        let paths: Vec<_> = frozen
            .manifest()
            .entries
            .iter()
            .filter(|e| e.method == Method::Stored)
            .map(|e| (e.path.clone(), e.role == Role::Index))
            .collect();
        let mut ranges = BTreeMap::new();
        for (path, index) in &paths {
            control.check()?;
            source.check()?;
            let extent = frozen.with_stored(path, |range| Ok(range.extent()))?;
            usize::try_from(extent.length).map_err(|_| Error::Inventory)?;
            ranges.insert(
                path.as_str().to_owned(),
                AdmittedRange {
                    extent,
                    index: *index,
                },
            );
        }
        frozen.revalidate()?;
        drop(frozen);
        drop(package_work);
        drop(paths);
        source.check()?;
        control.check()?;
        if Version::of(&source.file()?.metadata().map_err(|_| Error::Inventory)?) != version {
            return Err(Error::Inventory);
        }
        Ok(Self {
            backing: Arc::new(Backing {
                source,
                version,
                ranges,
                control: control.clone(),
                read_limit: Some(plan.read_window_bytes),
                reads: super::ReadLiveness::new(
                    plan.read_window_bytes
                        .checked_mul(4)
                        .ok_or(Error::Reservation)?,
                ),
                _work: Arc::new(Mutex::new(work)),
            }),
        })
    }
    /// Confined index-only component namespace. Handles retain the actual archive/credit owner.
    /// The caller separately prepays all buffers retained by its consuming engine.
    pub fn directory(&self, prefix: &str) -> Result<Arc<dyn ByteDirectory>, Error> {
        self.backing.check().map_err(|_| Error::Inventory)?;
        if prefix.starts_with('/')
            || (!prefix.is_empty()
                && prefix
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == ".."))
        {
            return Err(Error::Inventory);
        }
        // A real admitted member must bound the allocated prefix; no unknown-name probing.
        if !self.backing.ranges.iter().any(|(name, r)| {
            r.index
                && (prefix.is_empty()
                    || name
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('/')))
        }) {
            return Err(Error::Inventory);
        }
        Ok(Arc::new(ArchiveDirectory {
            backing: self.backing.clone(),
            prefix: if prefix.is_empty() {
                String::new()
            } else {
                format!("{prefix}/")
            },
            indexes_only: true,
        }))
    }
    /// Recheck actual source selection, policy, Claim/control and retained descriptor version.
    pub fn check(&self) -> Result<(), Error> {
        self.backing.check().map_err(|_| Error::Inventory)
    }
}
