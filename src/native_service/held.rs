//! Native caller composition for the committed M3 held-FD staging ABI; no publication authority is created.

use super::extraction::Inputs;
use super::{NativeAdmitted, NativeAdmittedError, NativeProcessing, NativeSource};
use crate::admitted;
use anyhow::{Result, ensure};
use knowledge_core::data::RelativePath;
use knowledge_core::jobs::Claim;
use knowledge_processing::held::{EpubMember, HeldEpub, HeldSource, HeldVideo};
use std::fs::File;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

/// One explicitly selected publication member; the vector order is the approved spine order.
pub struct NativeEpubMember {
    /// Actual current native source/permit/version owner, not a host path.
    pub source: NativeSource,
    /// Publication link name only; never reopened as a filesystem source.
    pub name: RelativePath,
}
/// Actual EPUB recipe and full generated extent; the caller binds these and ordered versions into its recipe digest.
pub struct NativeEpubPlan {
    /// Publication title.
    pub title: String,
    /// Include only selected referenced media members.
    pub include_media: bool,
    /// Opaque RSS promise; None uses only this actual Claim's remaining complete credit.
    pub child_rss_bytes: Option<u64>,
    /// Full prepaid staging output extent on the selected native volume.
    pub storage_bytes: u64,
}
/// Actual video recipe and complete child/output plan, not a new process budget.
pub struct NativeVideoPlan {
    /// Contiguous segment duration on the original recording clock.
    pub seconds: f64,
    /// Existing mature M3 bounds, including child RSS, generated extent and segment count.
    pub clips: knowledge_processing::media::ClipPlan,
}

/// Receipt for this exact exclusively created zero-length EPUB output; not permission or a guess
/// about generic unknown writers. Only whole native collection enables its one storage reconciliation.
pub(super) struct EpubWriteReceipt {
    file: File,
    identity: (u64, u64),
    maximum: u64,
    returned: Option<std::ops::Range<usize>>,
    reconciled: bool,
}
impl EpubWriteReceipt {
    fn new(file: &File, maximum: u64) -> Result<Self> {
        let m = file.metadata()?;
        ensure!(
            m.is_file() && m.len() == 0 && m.nlink() == 1,
            "EPUB output must be this owner's new empty inode"
        );
        Ok(Self {
            file: file.try_clone()?,
            identity: (m.dev(), m.ino()),
            maximum,
            returned: None,
            reconciled: false,
        })
    }
    pub(super) fn collected(&mut self, returned: std::ops::Range<usize>) {
        self.returned = Some(returned);
    }
    pub(super) fn reconcile(
        &mut self,
        retained: &mut [(knowledge_core::admission::Holding, bool)],
    ) -> Result<()> {
        if self.reconciled {
            return Ok(());
        }
        let range = self
            .returned
            .clone()
            .ok_or_else(|| anyhow::anyhow!("actual EPUB writer not wholly collected"))?;
        let records = retained
            .get_mut(range)
            .ok_or_else(|| anyhow::anyhow!("actual returned EPUB holdings missing"))?;
        let m = self.file.metadata()?;
        ensure!(
            m.is_file() && (m.dev(), m.ino()) == self.identity && m.len() <= self.maximum,
            "actual EPUB writer extent unresolved"
        );
        let mut storage = None;
        for (index, (credit, _)) in records.iter().enumerate() {
            if credit.remaining()?.temporary_bytes > 0 {
                ensure!(
                    storage.is_none() && credit.remaining()?.temporary_bytes == self.maximum,
                    "actual EPUB storage holding differs from its one writer"
                );
                storage = Some(index);
            }
        }
        let (credit, known) = records
            .get_mut(storage.ok_or_else(|| anyhow::anyhow!("actual EPUB storage credit absent"))?)
            .expect("actual located storage record");
        credit.written(m.len())?;
        *known = true;
        self.reconciled = true;
        Ok(())
    }
}

/// A real private staging namespace retained with its descriptor through last consumer/native collection.
pub(super) struct Scratch {
    pub directory: File,
    namespace: Option<tempfile::TempDir>,
}
impl Scratch {
    /// Require the original owned namespace before retirement; never clean a replacement name.
    pub(super) fn check(&self) -> Result<()> {
        let namespace = self.namespace.as_ref().expect("owned staging namespace");
        let named = std::fs::symlink_metadata(namespace.path())?;
        let held = self.directory.metadata()?;
        ensure!(
            named.is_dir()
                && !named.file_type().is_symlink()
                && (named.dev(), named.ino()) == (held.dev(), held.ino()),
            "native staging namespace changed; owner retained"
        );
        Ok(())
    }
}
impl Drop for Scratch {
    /// Never delete a replacement namespace after ancestor/name drift; retain that staging evidence instead.
    fn drop(&mut self) {
        if let Some(namespace) = self.namespace.take() {
            let same = std::fs::symlink_metadata(namespace.path())
                .ok()
                .zip(self.directory.metadata().ok())
                .is_some_and(|(named, held)| {
                    named.is_dir()
                        && !named.file_type().is_symlink()
                        && (named.dev(), named.ino()) == (held.dev(), held.ino())
                });
            if same {
                drop(namespace);
            } else {
                let _retained = namespace.keep();
            }
        }
    }
}
/// Create actual selected-volume scratch, proving by native ancestry that it is outside the selected source root.
fn scratch(
    processing: &NativeProcessing,
    claim: &Claim,
    inputs: &mut Inputs,
    metadata_bytes: u64,
) -> Result<Scratch> {
    processing.check(claim)?;
    let cache = admitted::cache_directory()?;
    // Prepay native path/temporary-name copies as well as caller member/record metadata.
    let path_bytes = (cache.as_os_str().as_encoded_bytes().len() as u64)
        .checked_add(64)
        .and_then(|bytes| bytes.checked_mul(16))
        .and_then(|bytes| bytes.checked_add(metadata_bytes))
        .ok_or_else(|| anyhow::anyhow!("native staging metadata overflow"))?;
    inputs
        .resources
        .push(knowledge_core::admission::Account::current()?.split_claim(
            claim,
            processing.key(),
            knowledge_core::jobs::Reservation {
                memory_bytes: path_bytes,
                temporary_bytes: 64 * 1024,
            },
        )?);
    // Use the already qualified private cache, but spend THIS Claim's namespace credit, not another reserve.
    let namespace = tempfile::Builder::new()
        .prefix("native-held-")
        .tempdir_in(cache)?;
    let directory = File::from(rustix::fs::open(
        namespace.path(),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    // Only this newly created owned namespace changes mode; never chmod a selected source or parent.
    directory.set_permissions(std::fs::Permissions::from_mode(0o700))?;
    let owned = Scratch {
        directory,
        namespace: Some(namespace),
    };
    let source = processing.source();
    source
        .permit()
        .with_root(source.library_id(), |root| {
            let root = root.metadata().map_err(super::refused)?;
            let mut current = owned.directory.try_clone().map_err(super::refused)?;
            loop {
                let held = current.metadata().map_err(super::refused)?;
                if (held.dev(), held.ino()) == (root.dev(), root.ino()) {
                    return Err(super::wire::ErrorCode::NotExposed);
                }
                let parent = File::from(
                    rustix::fs::openat(
                        &current,
                        "..",
                        rustix::fs::OFlags::RDONLY
                            | rustix::fs::OFlags::DIRECTORY
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::CLOEXEC,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(super::refused)?,
                );
                let info = parent.metadata().map_err(super::refused)?;
                if (held.dev(), held.ino()) == (info.dev(), info.ino()) {
                    return Ok(());
                }
                current = parent;
            }
        })
        .map_err(|_| anyhow::anyhow!("native staging placement refused"))?;
    source.check()?;
    Ok(owned)
}
impl NativeProcessing {
    /// Stage explicitly admitted members through M3's committed held-FD EPUB encoder.
    /// Ordered member versions, title and media options must describe the actual queued recipe.
    /// Native source checks cover every member before/after work and through the complete result lifetime.
    /// The returned FD is private staging only; user-visible publication still requires its existing owner.
    pub fn export_epub(
        self,
        claim: Claim,
        members: Vec<NativeEpubMember>,
        plan: NativeEpubPlan,
    ) -> std::result::Result<NativeAdmitted<HeldEpub>, NativeAdmittedError<HeldEpub>> {
        // The real owner exists BEFORE fallible placement/open work and keeps its Claim on errors.
        self.run(
            claim,
            Inputs::default(),
            move |processing, claim, inputs| encode_epub(processing, claim, inputs, members, plan),
        )
    }
    /// Stage contiguous clips from the exact admitted source/version into an actual private directory FD.
    /// No source/output path is reopened, no generated clip is consumed by name, and the original source clock is retained.
    pub fn segment_video(
        self,
        claim: Claim,
        plan: NativeVideoPlan,
    ) -> std::result::Result<NativeAdmitted<HeldVideo>, NativeAdmittedError<HeldVideo>> {
        self.run(claim, Inputs::default(), |processing, claim, inputs| {
            encode_video(processing, claim, inputs, plan)
        })
    }
}

pub(super) fn encode_epub(
    processing: &NativeProcessing,
    claim: &mut Claim,
    inputs: &mut Inputs,
    members: Vec<NativeEpubMember>,
    plan: NativeEpubPlan,
) -> Result<knowledge_processing::Admitted<HeldEpub>> {
    let metadata = members
        .iter()
        .try_fold(plan.title.len() as u64 + 4096, |total, member| {
            total
                .checked_add(member.name.as_str().len() as u64)
                .and_then(|n| {
                    n.checked_add(
                        (std::mem::size_of::<NativeEpubMember>()
                            + std::mem::size_of::<EpubMember<'_>>()) as u64
                            * 2,
                    )
                })
                .ok_or_else(|| anyhow::anyhow!("native member plan overflow"))
        })?;
    inputs._scratch = Some(scratch(processing, claim, inputs, metadata)?);
    let directory = &inputs
        ._scratch
        .as_ref()
        .expect("owned staging namespace")
        .directory;
    inputs.leases.push(File::from(rustix::fs::openat(
        directory,
        "publication.epub",
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )?));
    inputs.epub_writer = Some(EpubWriteReceipt::new(
        &inputs.leases[0],
        plan.storage_bytes,
    )?);
    let mut names = Vec::with_capacity(members.len());
    inputs.sources.reserve_exact(members.len());
    for member in members {
        if inputs.publication.is_some() {
            // This actual result owner keeps every separately prepaid source through publication.
            // A duplicate anchor delivery pin would reserve from the global account after claim.
            member.source.check()?;
            ensure!(
                member.source.library_id() == processing.source().library_id()
                    && member.source.context() == processing.source().context(),
                "planned publication members require the same actual source context"
            );
        } else {
            processing.source().retain_member(&member.source)?;
        }
        names.push(member.name);
        inputs.sources.push(member.source);
    }
    let views = inputs
        .sources
        .iter()
        .zip(&names)
        .map(|(source, name)| {
            Ok(EpubMember {
                source: HeldSource {
                    file: source.file()?,
                    expected_version: source.version(),
                },
                logical_name: name,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    knowledge_processing::epub::export_epub_held_funded(
        &views,
        &inputs.leases[0],
        &plan.title,
        plan.include_media,
        plan.child_rss_bytes,
        plan.storage_bytes,
        &mut processing.funding(claim)?,
    )
}

/// One exact M3 video call and private directory, shared by legacy staging and native publication.
pub(super) fn encode_video(
    processing: &NativeProcessing,
    claim: &mut Claim,
    inputs: &mut Inputs,
    plan: NativeVideoPlan,
) -> Result<knowledge_processing::Admitted<HeldVideo>> {
    inputs._scratch = Some(scratch(processing, claim, inputs, 4096)?);
    let source = processing.source();
    ensure!(
        source.file()?.metadata()?.is_file(),
        "native media source requires a regular file"
    );
    if inputs.publication.is_some() {
        inputs.video_writer = Some(VideoWriteReceipt::new(
            &inputs._scratch.as_ref().expect("owned scratch").directory,
            source.file()?,
            plan.clips.storage_bytes,
            plan.clips.max_segments,
        )?);
    }
    knowledge_processing::media::segment_video_held_funded(
        HeldSource {
            file: source.file()?,
            expected_version: source.version(),
        },
        &inputs
            ._scratch
            .as_ref()
            .expect("owned staging namespace")
            .directory,
        plan.seconds,
        plan.clips,
        &mut processing.funding(claim)?,
    )
}
/// Receipt for the one exact M3 call in an owner-created initially empty clip directory.
/// It reconciles only after native collection proves the whole group and pipes are stopped.
pub(super) struct VideoWriteReceipt {
    directory: File,
    directory_id: (u64, u64),
    source_id: (u64, u64),
    maximum: u64,
    maximum_count: usize,
    returned: Option<std::ops::Range<usize>>,
    reconciled: bool,
}
impl VideoWriteReceipt {
    fn new(directory: &File, source: &File, maximum: u64, maximum_count: usize) -> Result<Self> {
        let d = directory.metadata()?;
        let s = source.metadata()?;
        ensure!(d.is_dir(), "video receipt requires actual directory");
        for entry in rustix::fs::Dir::read_from(directory)? {
            let entry = entry?;
            let name = entry.file_name().to_bytes();
            ensure!(
                name == b"." || name == b"..",
                "video receipt requires this call's empty directory"
            );
        }
        Ok(Self {
            directory: directory.try_clone()?,
            directory_id: (d.dev(), d.ino()),
            source_id: (s.dev(), s.ino()),
            maximum,
            maximum_count,
            returned: None,
            reconciled: false,
        })
    }
    pub(super) fn collected(&mut self, returned: std::ops::Range<usize>) {
        self.returned = Some(returned);
    }
    fn snapshot(&self, descriptors: &[File]) -> Result<Vec<(String, [u64; 7])>> {
        let d = self.directory.metadata()?;
        ensure!(
            (d.dev(), d.ino()) == self.directory_id,
            "actual video directory changed"
        );
        let mut outputs = std::collections::BTreeSet::new();
        for file in descriptors {
            let m = file.metadata()?;
            let id = (m.dev(), m.ino());
            if id == self.directory_id || id == self.source_id {
                continue;
            }
            ensure!(
                m.is_file() && m.nlink() == 1 && outputs.len() < self.maximum_count,
                "foreign video descriptor or excessive outputs"
            );
            ensure!(outputs.insert(id), "duplicate video output descriptor");
        }
        let mut snapshot = Vec::new();
        for entry in rustix::fs::Dir::read_from(&self.directory)? {
            let entry = entry?;
            let raw = entry.file_name().to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            ensure!(
                snapshot.len() < self.maximum_count,
                "unknown video directory entries"
            );
            let name = std::str::from_utf8(raw)?;
            let file = File::from(rustix::fs::openat(
                &self.directory,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )?);
            let m = file.metadata()?;
            let id = (m.dev(), m.ino());
            ensure!(
                m.is_file() && m.nlink() == 1,
                "foreign video directory member"
            );
            ensure!(
                outputs.remove(&id),
                "video member lacks this actual child descriptor"
            );
            snapshot.push((
                name.to_owned(),
                [
                    m.dev(),
                    m.ino(),
                    m.len(),
                    m.mtime() as u64,
                    m.mtime_nsec() as u64,
                    m.ctime() as u64,
                    m.ctime_nsec() as u64,
                ],
            ));
        }
        ensure!(
            outputs.is_empty(),
            "actual video output lost its owned directory name"
        );
        snapshot.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(snapshot)
    }
    pub(super) fn reconcile(
        &mut self,
        retained: &mut [(knowledge_core::admission::Holding, bool)],
        descriptors: &[File],
    ) -> Result<()> {
        if self.reconciled {
            return Ok(());
        }
        let range = self
            .returned
            .clone()
            .ok_or_else(|| anyhow::anyhow!("video group not collected"))?;
        let records = retained
            .get_mut(range)
            .ok_or_else(|| anyhow::anyhow!("actual video holdings absent"))?;
        let before = self.snapshot(descriptors)?;
        let total = before.iter().try_fold(0u64, |sum, (_, m)| {
            sum.checked_add(m[2])
                .ok_or_else(|| anyhow::anyhow!("video extent overflow"))
        })?;
        ensure!(
            total <= self.maximum,
            "actual video extent exceeds its promise"
        );
        let mut storage = None;
        for (i, (holding, _)) in records.iter().enumerate() {
            let r = holding.remaining()?;
            if r.memory_bytes == 0 {
                ensure!(
                    storage.is_none() && r.temporary_bytes <= self.maximum,
                    "ambiguous actual video storage credit"
                );
                storage = Some(i);
            }
        }
        let (credit, known) = records
            .get_mut(storage.ok_or_else(|| anyhow::anyhow!("actual video storage credit missing"))?)
            .expect("located actual storage");
        let accounted = self
            .maximum
            .checked_sub(credit.remaining()?.temporary_bytes)
            .ok_or_else(|| anyhow::anyhow!("video storage promise changed"))?;
        let delta = total
            .checked_sub(accounted)
            .ok_or_else(|| anyhow::anyhow!("video accounted extent exceeds actual outputs"))?;
        ensure!(
            self.snapshot(descriptors)? == before,
            "video collection membership changed"
        );
        credit.written(delta)?;
        *known = true;
        self.reconciled = true;
        Ok(())
    }
}
