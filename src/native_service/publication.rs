//! Native saved EPUB plan and descriptor-owned publication inside the existing result owner.
use super::*;
use crate::native_service::{NativeEpubMember, NativeEpubPlan};
use anyhow::{Context, ensure};
use knowledge_core::jobs::Reservation;
use knowledge_processing::extract::Extraction;
use knowledge_processing::held::{HeldEpub, HeldVideo};
use knowledge_retrieval::Work;
use omni_service::authority::{Owner, Permit, Selection, SelectionLease};
use omni_service::{Command, Id};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt};

const WINDOW: usize = 64 * 1024;
const PUBLICATION_MEMORY: u64 = 256 * 1024;

/// An actual owner-selected, paid saved plan. Its ID/recipe are data, never constructors.
/// Native callers retain this object from planning through the matching export operation.
pub struct NativePublicationPlan {
    id: Id,
    recipe: knowledge_core::data::ContentDigest,
    destination: Permit,
    lease: SelectionLease,
    parent: File,
    names: Vec<String>,
    derived: Option<NativeDerivedRecipe>,
    source_binding: Option<(Id, String, knowledge_core::data::ContentDigest)>,
    members: Vec<NativeEpubMember>,
    epub: Option<NativeEpubPlan>,
    maximum: u64,
    scratch_memory: u64,
    #[cfg(test)]
    after_install: Option<(usize, Box<dyn FnOnce() + Send>)>,
    #[cfg(test)]
    install_fault: Option<usize>,
    // All plan strings, source/selected-root guards and FDs die before their actual Work.
    _metadata: Work,
}
fn same(a: &File, b: &File) -> Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok((a.dev(), a.ino()) == (b.dev(), b.ino()))
}
fn output_version(file: &File) -> Result<[u64; 7]> {
    let m = file.metadata()?;
    ensure!(m.is_file(), "held publication artifact is not regular");
    Ok([
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        m.ctime() as u64,
        m.ctime_nsec() as u64,
    ])
}
fn regular(parent: &File, name: &str) -> Result<File> {
    let file = File::from(rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?);
    ensure!(
        file.metadata()?.is_file(),
        "publication name is not a regular file"
    );
    Ok(file)
}
impl NativePublicationPlan {
    /// Quote retained plan metadata before allocating/copying the native saved plan.
    pub fn metadata_requirement(
        members: &Vec<NativeEpubMember>,
        recipe: &NativeEpubPlan,
        final_name: &str,
    ) -> Result<Reservation> {
        let mut bytes = (final_name.len() as u64)
            .checked_add(recipe.title.capacity() as u64)
            .and_then(|n| {
                n.checked_add(
                    (members.capacity() as u64)
                        .checked_mul(std::mem::size_of::<NativeEpubMember>() as u64)?,
                )
            })
            .context("publication metadata overflow")?;
        for member in members {
            bytes = bytes
                .checked_add(member.name.as_str().len() as u64)
                .and_then(|n| n.checked_add(member.source.hint().as_os_str().len() as u64))
                .and_then(|n| n.checked_add(1024))
                .context("publication metadata overflow")?;
        }
        Ok(Reservation {
            memory_bytes: bytes
                .checked_mul(32)
                .and_then(|n| n.checked_add(PUBLICATION_MEMORY))
                .context("publication metadata overflow")?,
            temporary_bytes: 0,
        })
    }
    /// Bind an actual human-approved EpubPlan to a separately selected destination root.
    /// The final name is one native root-relative filename; wire paths never select its parent.
    pub fn epub(
        owner: &Owner,
        destination_selection: &Selection,
        destination_plan_permit: Permit,
        final_name: &str,
        mut members: Vec<NativeEpubMember>,
        recipe: NativeEpubPlan,
        metadata: Work,
    ) -> Result<Self> {
        ensure!(
            metadata.remaining()?.memory_bytes
                >= Self::metadata_requirement(&members, &recipe, final_name)?.memory_bytes,
            "native publication plan metadata is not prepaid"
        );
        ensure!(
            !final_name.is_empty()
                && final_name != "."
                && final_name != ".."
                && !final_name.contains(['/', '\\', '\0']),
            "publication requires a single final filename"
        );
        let lease = owner
            .pin_selection(destination_selection)
            .map_err(crate::native_service::source::refused_error)?;
        lease
            .check_permit(&destination_plan_permit)
            .map_err(crate::native_service::source::refused_error)?;
        let Command::EpubPlan {
            library_id,
            page_paths,
            media_source_ids,
            destination,
            budget,
        } = destination_plan_permit
            .admitted_command()
            .map_err(crate::native_service::source::refused_error)?
        else {
            anyhow::bail!("actual EpubPlan destination write intention required");
        };
        ensure!(
            destination == final_name
                && !page_paths.is_empty()
                && members.len() == page_paths.len() + media_source_ids.len()
                && (recipe.include_media || media_source_ids.is_empty())
                && members.len() as u64 <= budget.max_files.get()
                && recipe.storage_bytes > 0
                && recipe.storage_bytes <= budget.max_bytes.get(),
            "publication differs from actual native plan selection/budget"
        );
        for member in &mut members {
            member.name = knowledge_core::data::RelativePath::new(member.name.as_str())?;
        }
        for (index, member) in members.iter().enumerate() {
            member.source.check()?;
            ensure!(
                member.source.library_id() == library_id
                    && member
                        .source
                        .permit()
                        .admitted_command()
                        .map_err(crate::native_service::source::refused_error)?
                        == destination_plan_permit
                            .admitted_command()
                            .map_err(crate::native_service::source::refused_error)?,
                "publication source lacks the actual matching EpubPlan intention"
            );
            if let Some(path) = page_paths.get(index) {
                ensure!(
                    member.source.hint() == std::path::Path::new(path.as_str()),
                    "publication page order changed"
                );
            } else {
                ensure!(
                    member.source.id() == &media_source_ids[index - page_paths.len()],
                    "publication media selection changed"
                );
            }
        }
        let (parent, destination_revision) = destination_plan_permit
            .with_selected_context(library_id, |root, _, revision| {
                root.try_clone()
                    .map(|root| (root, revision))
                    .map_err(crate::native_service::refused)
            })
            .map_err(crate::native_service::source::refused_error)?;
        ensure!(
            parent.metadata()?.dev()
                == Account::current()?.generated_directory()?.metadata()?.dev(),
            "publication destination differs from the one accounted native volume"
        );
        let mut hash = Sha256::new();
        hash.update(b"omni-native-epub-publication-v1\0");
        let target = parent.metadata()?;
        hash.update(target.dev().to_le_bytes());
        hash.update(target.ino().to_le_bytes());
        hash.update(destination_revision.to_le_bytes());
        hash.update(serde_json::to_vec(
            destination_plan_permit
                .admitted_command()
                .map_err(crate::native_service::source::refused_error)?,
        )?);
        for member in &members {
            for value in [
                member.name.as_str(),
                member.source.id().as_str(),
                member.source.context(),
                member.source.version().as_str(),
            ] {
                hash.update((value.len() as u64).to_le_bytes());
                hash.update(value.as_bytes());
            }
        }
        hash.update(recipe.title.as_bytes());
        hash.update(recipe.storage_bytes.to_le_bytes());
        hash.update([u8::from(recipe.include_media)]);
        hash.update(recipe.child_rss_bytes.unwrap_or(0).to_le_bytes());
        let digest = knowledge_core::data::ContentDigest::new(hex::encode(hash.finalize()))
            .map_err(|_| anyhow::anyhow!("native plan digest invalid"))?;
        let cache = crate::admitted::cache_directory()?;
        let scratch_memory = (cache.as_os_str().len() as u64)
            .checked_add(64)
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_add(recipe.title.len() as u64 + 4096))
            .context("publication native scratch overflow")?;
        let scratch_memory = members.iter().try_fold(scratch_memory, |total, member| {
            total
                .checked_add(member.name.as_str().len() as u64)
                .and_then(|n| {
                    n.checked_add(
                        ((std::mem::size_of::<NativeEpubMember>()
                            + std::mem::size_of::<knowledge_processing::held::EpubMember<'_>>())
                            * 2) as u64,
                    )
                })
                .context("publication native scratch overflow")
        })?;
        Ok(Self {
            id: Id::new(uuid::Uuid::new_v4().to_string())?,
            recipe: digest,
            maximum: recipe.storage_bytes,
            scratch_memory,
            epub: Some(recipe),
            destination: destination_plan_permit,
            lease,
            parent,
            names: vec![final_name.to_owned()],
            derived: None,
            source_binding: None,
            members,
            #[cfg(test)]
            after_install: None,
            #[cfg(test)]
            install_fault: None,
            _metadata: metadata,
        })
    }
    /// Opaque plan's observed ID, for routing the existing EpubExport command.
    pub fn id(&self) -> &Id {
        &self.id
    }
    /// Complete ordered source/version/native recipe digest for the existing queued work key.
    pub fn recipe(&self) -> &knowledge_core::data::ContentDigest {
        &self.recipe
    }
    /// Compose the complete simultaneous publication sibling BEFORE existing queue submission.
    /// Actual same-Claim split still verifies this promise before processing starts.
    pub fn requirement(&self, processing: Reservation) -> Result<Reservation> {
        Ok(Reservation {
            memory_bytes: processing
                .memory_bytes
                .checked_add(PUBLICATION_MEMORY)
                .and_then(|n| n.checked_add(self.scratch_memory))
                .context("publication peak overflow")?,
            temporary_bytes: processing
                .temporary_bytes
                .checked_add(self.maximum)
                .and_then(|n| n.checked_add(8192 + 64 * 1024))
                .context("publication peak overflow")?,
        })
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_after_install(
        &mut self,
        hook: impl FnOnce() + Send + 'static,
    ) {
        self.test_after_install_at(0, hook);
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_install_fault(&mut self) {
        self.test_install_fault_at(0);
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_after_install_at(
        &mut self,
        index: usize,
        hook: impl FnOnce() + Send + 'static,
    ) {
        self.after_install = Some((index, Box::new(hook)));
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_install_fault_at(&mut self, index: usize) {
        self.install_fault = Some(index);
    }
    fn check(&self) -> Result<()> {
        self.lease
            .check_permit(&self.destination)
            .map_err(crate::native_service::source::refused_error)?;
        let library = match self
            .destination
            .admitted_command()
            .map_err(crate::native_service::source::refused_error)?
        {
            Command::EpubPlan { library_id, .. }
            | Command::EpubExport { library_id, .. }
            | Command::Process { library_id, .. } => library_id,
            _ => anyhow::bail!("publication lost actual write intention"),
        };
        self.destination
            .with_selected_context(library, |root, _, _| {
                if same(root, &self.parent).unwrap_or(false) {
                    Ok(())
                } else {
                    Err(omni_service::ErrorCode::Changed)
                }
            })
            .map_err(crate::native_service::source::refused_error)?;
        ensure!(
            self.parent.metadata()?.dev()
                == Account::current()?.generated_directory()?.metadata()?.dev(),
            "publication volume changed"
        );
        Ok(())
    }
}
/// Borrow one actual paid output without another whole-output buffer or temporary file.
enum Output<'a> {
    File(&'a File),
    Bytes(&'a [u8]),
}
impl Output<'_> {
    fn version(&self) -> Result<[u64; 7]> {
        match self {
            Self::File(f) => output_version(f),
            Self::Bytes(b) => Ok([0, 0, b.len() as u64, 0, 0, 0, 0]),
        }
    }
    fn read_at(&self, out: &mut [u8], offset: u64) -> Result<()> {
        match self {
            Self::File(f) => f.read_exact_at(out, offset)?,
            Self::Bytes(b) => {
                let start = usize::try_from(offset)?;
                out.copy_from_slice(
                    b.get(
                        start
                            ..start
                                .checked_add(out.len())
                                .context("output offset overflow")?,
                    )
                    .context("output range changed")?,
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prepared,
    Copying,
    Installed,
    Durable,
}
pub(in crate::native_service) struct Publication {
    plan: NativePublicationPlan,
    stage: Option<(String, File)>,
    phase: Phase,
    accounted: u64,
    total: u64,
    index: usize,
    durable: Vec<(usize, File, [u64; 7])>,
    // Actual copied/staged extent and copy buffer remain paid through settlement.
    work: Work,
}
impl Publication {
    pub(super) fn check(&self) -> Result<()> {
        self.plan.check()?;
        for (index, held, version) in &self.durable {
            ensure!(
                same(&regular(&self.plan.parent, &self.plan.names[*index])?, held)?
                    && output_version(held)? == *version,
                "earlier installed output identity changed"
            );
        }
        Ok(())
    }
    pub(super) fn require_installed(&self) -> Result<()> {
        ensure!(
            self.phase == Phase::Durable && self.index + 1 == self.plan.names.len(),
            "native publication has not completed"
        );
        Ok(())
    }
    fn reconcile(&mut self) -> Result<()> {
        if let Some((_, file)) = &self.stage {
            let length = file.metadata()?.len();
            let delta = length
                .checked_sub(self.accounted)
                .context("publication extent regressed")?;
            self.work.written(delta)?;
            self.accounted = length;
            self.total = self
                .total
                .checked_add(delta)
                .context("publication total overflow")?;
        }
        Ok(())
    }
    // Finish started effects using actual phase/FDs; revoked permits cannot prevent collection.
    pub(super) fn collect(&mut self) -> Result<()> {
        self.reconcile()?;
        let Some((stage_name, held)) = &self.stage else {
            return Ok(());
        };
        if matches!(self.phase, Phase::Installed | Phase::Durable) {
            ensure!(
                same(
                    &regular(&self.plan.parent, &self.plan.names[self.index])?,
                    held
                )?,
                "installed publication identity remains unresolved"
            );
            self.plan.parent.sync_all()?;
        }
        match regular(&self.plan.parent, stage_name) {
            Ok(named) => {
                ensure!(
                    same(&named, held)?,
                    "publication staging identity remains unresolved"
                );
                rustix::fs::unlinkat(
                    &self.plan.parent,
                    stage_name.as_str(),
                    rustix::fs::AtFlags::empty(),
                )?;
                self.plan.parent.sync_all()?;
            }
            Err(error)
                if error.downcast_ref::<rustix::io::Errno>() == Some(&rustix::io::Errno::NOENT) => {
            }
            Err(error) => return Err(error),
        }
        let version = if self.phase == Phase::Installed {
            Some(output_version(held)?)
        } else {
            None
        };
        if let Some((_, held)) = self.stage.take()
            && let Some(version) = version
        {
            self.durable.push((self.index, held, version));
        }
        if self.phase == Phase::Installed {
            self.phase = Phase::Durable;
        }
        Ok(())
    }
    fn publish(&mut self, source: &File, current: impl FnMut() -> Result<()>) -> Result<()> {
        self.publish_input(Output::File(source), current)
    }
    fn publish_input(
        &mut self,
        source: Output<'_>,
        mut current: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        if self.phase == Phase::Durable && self.index + 1 < self.plan.names.len() {
            self.index += 1;
            self.accounted = 0;
            self.phase = Phase::Prepared;
        }
        ensure!(
            self.phase == Phase::Prepared,
            "publication cannot be replayed"
        );
        current()?;
        self.check()?;
        let before = source.version()?;
        let length = before[2];
        ensure!(
            length
                <= self
                    .plan
                    .maximum
                    .checked_sub(self.total)
                    .context("publication total exceeds promise")?,
            "publication exceeds its prepaid complete output extent"
        );
        let name = format!(".omni-publication-{}", uuid::Uuid::new_v4());
        let file = File::from(rustix::fs::openat(
            &self.plan.parent,
            name.as_str(),
            rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::from_bits_truncate(0o600),
        )?);
        self.stage = Some((name, file));
        self.phase = Phase::Copying;
        let mut buffer = vec![0; WINDOW];
        let mut offset = 0;
        while offset < length {
            current()?;
            self.plan.check()?;
            let n = (length - offset).min(WINDOW as u64) as usize;
            ensure!(
                source.version()? == before,
                "held publication artifact changed"
            );
            source.read_at(&mut buffer[..n], offset)?;
            ensure!(
                source.version()? == before,
                "held publication artifact changed"
            );
            current()?;
            self.plan.check()?;
            let result = self
                .stage
                .as_mut()
                .expect("actual publication stage")
                .1
                .write_all(&buffer[..n]);
            self.reconcile()?;
            result?;
            offset += n as u64;
        }
        current()?;
        self.plan.check()?;
        let (name, held) = self.stage.as_ref().expect("actual publication stage");
        held.sync_all()?;
        ensure!(
            same(&regular(&self.plan.parent, name)?, held)?,
            "publication stage replaced"
        );
        current()?;
        self.plan.check()?;
        ensure!(
            source.version()? == before,
            "held publication artifact changed"
        );
        rustix::fs::linkat(
            &self.plan.parent,
            name.as_str(),
            &self.plan.parent,
            self.plan.names[self.index].as_str(),
            rustix::fs::AtFlags::empty(),
        )?;
        self.phase = Phase::Installed; // Before every fallible observation/sync following installation.
        #[cfg(test)]
        if self
            .plan
            .after_install
            .as_ref()
            .is_some_and(|(index, _)| *index == self.index)
            && let Some((_, hook)) = self.plan.after_install.take()
        {
            hook();
        }
        #[cfg(test)]
        if self.plan.install_fault == Some(self.index) {
            self.plan.install_fault = None;
            anyhow::bail!("injected publication observation failure");
        }
        self.collect()
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        while self.collect().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        self.durable = Vec::new();
    }
}
impl NativeProcessing {
    /// Consume the exact owner-created plan under the existing actual EpubExport intention.
    /// The complete publication sibling is split before M3 computes any remaining RSS allowance.
    pub fn export_epub_planned(
        self,
        claim: Claim,
        mut plan: NativePublicationPlan,
        actual_export_permit: Permit,
    ) -> std::result::Result<NativeAdmitted<HeldEpub>, NativeAdmittedError<HeldEpub>> {
        self.run(
            claim,
            Inputs::default(),
            move |processing, claim, inputs| {
                plan.check()?;
                plan.lease
                    .check_permit(&actual_export_permit)
                    .map_err(crate::native_service::source::refused_error)?;
                let Command::EpubExport {
                    library_id,
                    plan_id,
                    ..
                } = actual_export_permit
                    .admitted_command()
                    .map_err(crate::native_service::source::refused_error)?
                else {
                    anyhow::bail!("actual EpubExport write intention required");
                };
                ensure!(
                    plan_id == plan.id()
                        && library_id == processing.source().library_id()
                        && processing.key().recipe == plan.recipe(),
                    "publication does not own this actual queued plan"
                );
                let Command::EpubPlan { budget, .. } = plan
                    .destination
                    .admitted_command()
                    .map_err(crate::native_service::source::refused_error)?
                else {
                    anyhow::bail!("actual saved EPUB plan required");
                };
                let actual = claim.budget();
                ensure!(
                    actual.max_bytes <= budget.max_bytes.get()
                        && actual.max_files <= budget.max_files.get()
                        && actual.max_millis <= budget.max_millis.get()
                        && actual.max_memory_bytes <= budget.max_memory_bytes.get(),
                    "export exceeds the actual saved plan budget"
                );
                ensure!(
                    processing
                        .source()
                        .permit()
                        .admitted_command()
                        .map_err(crate::native_service::source::refused_error)?
                        == plan
                            .destination
                            .admitted_command()
                            .map_err(crate::native_service::source::refused_error)?,
                    "export anchor lacks actual saved plan scope"
                );
                let work = Work::split_claim(
                    claim,
                    processing.key(),
                    Reservation {
                        memory_bytes: PUBLICATION_MEMORY,
                        temporary_bytes: plan
                            .maximum
                            .checked_add(8192)
                            .context("publication peak overflow")?,
                    },
                )?;
                let members = std::mem::take(&mut plan.members);
                let recipe = plan.epub.take().context("actual saved EPUB recipe")?;
                plan.destination = actual_export_permit;
                let durable = Vec::with_capacity(plan.names.len());
                inputs.publication = Some(Publication {
                    durable,
                    plan,
                    stage: None,
                    phase: Phase::Prepared,
                    accounted: 0,
                    total: 0,
                    index: 0,
                    work,
                });
                crate::native_service::held::encode_epub(processing, claim, inputs, members, recipe)
            },
        )
    }
}
impl NativeAdmitted<HeldEpub> {
    /// Publish the held artifact through this same result's prepaid, selected destination owner.
    /// An uncertain install returns the same owning native error, never a static residue counter.
    pub fn publish(mut self) -> std::result::Result<Self, NativeAdmittedError<HeldEpub>> {
        let result = (|| {
            self.value()?;
            let processing = self
                .processing
                .as_ref()
                .context("actual native processing owner")?;
            let claim = self.claim.as_ref().context("actual native Claim")?;
            let output = &self.output.as_ref().context("actual held EPUB")?.value.file;
            let sources = &self.inputs.sources;
            self.inputs
                .publication
                .as_mut()
                .context("actual saved publication owner required")?
                .publish(output, || {
                    processing.check(claim)?;
                    for source in sources {
                        source.check()?;
                    }
                    Ok(())
                })
        })();
        match result {
            Ok(()) => Ok(self),
            Err(cause) => Err(NativeAdmittedError {
                cause,
                owner: Box::new(self),
            }),
        }
    }
}
/// Native derived output recipe; explicit destination names belong to the saved owner, not wire paths.
pub enum NativeDerivedRecipe {
    /// Bounded actual text extraction.
    Text {
        max_bytes: usize,
        child_rss_bytes: Option<u64>,
    },
    /// Contiguous original-clock clips with one explicit native final name per expected clip.
    Video(crate::native_service::NativeVideoPlan),
}
impl NativePublicationPlan {
    /// Quote copied destination names and source binding before constructing a derived plan.
    pub fn derived_metadata_requirement(
        source: &NativeSource,
        names: &Vec<String>,
    ) -> Result<Reservation> {
        let bytes = names.iter().try_fold(
            4096u64
                .checked_add(source.context().len() as u64)
                .and_then(|n| n.checked_add(source.id().as_str().len() as u64))
                .and_then(|n| {
                    n.checked_add(
                        (names.capacity() as u64)
                            .checked_mul(std::mem::size_of::<String>() as u64)?,
                    )
                })
                .context("derived plan overflow")?,
            |n, s| {
                n.checked_add(s.capacity() as u64)
                    .and_then(|n| n.checked_add(256))
                    .context("derived plan overflow")
            },
        )?;
        Ok(Reservation {
            memory_bytes: bytes
                .checked_mul(32)
                .and_then(|n| n.checked_add(PUBLICATION_MEMORY))
                .context("derived plan overflow")?,
            temporary_bytes: 0,
        })
    }
    /// Bind a real selected Process intention, source/version and separate destination selection.
    /// The same owner must approve the actual destination Process command; library labels alone do not qualify roots.
    pub fn derived(
        owner: &Owner,
        selection: &Selection,
        permit: Permit,
        source: &NativeSource,
        names: Vec<String>,
        derived: NativeDerivedRecipe,
        metadata: Work,
    ) -> Result<Self> {
        ensure!(
            metadata.remaining()?.memory_bytes
                >= Self::derived_metadata_requirement(source, &names)?.memory_bytes,
            "derived plan metadata not prepaid"
        );
        ensure!(
            !names.is_empty(),
            "derived publication requires explicit final names"
        );
        let mut unique = std::collections::BTreeSet::new();
        for name in &names {
            ensure!(
                !name.is_empty()
                    && name != "."
                    && name != ".."
                    && !name.contains(['/', '\\', '\0'])
                    && unique.insert(name.as_str()),
                "derived final names must be unique confined filenames"
            );
        }
        let lease = owner
            .pin_selection(selection)
            .map_err(crate::native_service::source::refused_error)?;
        lease
            .check_permit(&permit)
            .map_err(crate::native_service::source::refused_error)?;
        source.check()?;
        let command = permit
            .admitted_command()
            .map_err(crate::native_service::source::refused_error)?;
        let Command::Process {
            library_id,
            source_ids,
            recipe,
            budget,
            ..
        } = command
        else {
            anyhow::bail!("actual Process write intention required");
        };
        ensure!(
            library_id == source.library_id()
                && source_ids.len() == 1
                && source_ids[0] == *source.id()
                && source
                    .permit()
                    .admitted_command()
                    .map_err(crate::native_service::source::refused_error)?
                    == command,
            "derived source differs from actual selected intention"
        );
        let mut hash = Sha256::new();
        hash.update(b"omni-native-derived-publication-v1\0");
        hash.update(serde_json::to_vec(command)?);
        let (maximum, scratch_memory) = match &derived {
            NativeDerivedRecipe::Text {
                max_bytes,
                child_rss_bytes,
            } => {
                ensure!(
                    *recipe == omni_service::ProcessRecipe::Extract
                        && names.len() == 1
                        && *max_bytes > 0
                        && *max_bytes as u64 <= budget.max_bytes.get(),
                    "text recipe differs from approved Process scope"
                );
                hash.update((*max_bytes as u64).to_le_bytes());
                hash.update(child_rss_bytes.unwrap_or(0).to_le_bytes());
                (*max_bytes as u64, 0)
            }
            NativeDerivedRecipe::Video(plan) => {
                ensure!(
                    *recipe == omni_service::ProcessRecipe::Media
                        && plan.seconds.is_finite()
                        && plan.seconds > 0.0
                        && plan.clips.max_segments == names.len()
                        && plan.clips.max_segments <= 100000
                        && plan.clips.storage_bytes > 0
                        && plan.clips.storage_bytes <= budget.max_bytes.get(),
                    "video recipe differs from approved Process scope"
                );
                hash.update(plan.seconds.to_bits().to_le_bytes());
                hash.update(plan.clips.storage_bytes.to_le_bytes());
                hash.update((plan.clips.max_segments as u64).to_le_bytes());
                hash.update(plan.clips.child_rss_bytes.unwrap_or(0).to_le_bytes());
                let cache = crate::admitted::cache_directory()?;
                (
                    plan.clips.storage_bytes,
                    (cache.as_os_str().len() as u64 + 64)
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(4096))
                        .context("video scratch overflow")?,
                )
            }
        };
        ensure!(
            names.len() as u64 <= budget.max_files.get(),
            "derived names exceed approved count"
        );
        let (parent, revision) = permit
            .with_selected_context(library_id, |root, _, rev| {
                root.try_clone()
                    .map(|r| (r, rev))
                    .map_err(crate::native_service::refused)
            })
            .map_err(crate::native_service::source::refused_error)?;
        let m = parent.metadata()?;
        ensure!(
            m.dev() == Account::current()?.generated_directory()?.metadata()?.dev(),
            "derived destination differs from accounted volume"
        );
        hash.update(m.dev().to_le_bytes());
        hash.update(m.ino().to_le_bytes());
        hash.update(revision.to_le_bytes());
        for value in names.iter().map(String::as_str).chain([
            source.id().as_str(),
            source.context(),
            source.version().as_str(),
        ]) {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        Ok(Self {
            id: Id::new(uuid::Uuid::new_v4().to_string())?,
            recipe: knowledge_core::data::ContentDigest::new(hex::encode(hash.finalize()))?,
            destination: permit,
            lease,
            parent,
            names,
            derived: Some(derived),
            source_binding: Some((
                source.id().clone(),
                source.context().to_owned(),
                source.version().clone(),
            )),
            members: vec![],
            epub: None,
            maximum,
            scratch_memory,
            #[cfg(test)]
            after_install: None,
            #[cfg(test)]
            install_fault: None,
            _metadata: metadata,
        })
    }
    fn start_derived(
        mut self,
        processing: &NativeProcessing,
        claim: &Claim,
    ) -> Result<(Publication, NativeDerivedRecipe)> {
        self.check()?;
        let source = processing.source();
        ensure!(
            self.source_binding
                .as_ref()
                .is_some_and(|(id, context, version)| id == source.id()
                    && context == source.context()
                    && version == source.version())
                && processing.key().recipe == self.recipe()
                && source
                    .permit()
                    .admitted_command()
                    .map_err(crate::native_service::source::refused_error)?
                    == self
                        .destination
                        .admitted_command()
                        .map_err(crate::native_service::source::refused_error)?,
            "derived dispatch differs from actual queued plan/source"
        );
        let Command::Process { budget, .. } = self
            .destination
            .admitted_command()
            .map_err(crate::native_service::source::refused_error)?
        else {
            anyhow::bail!("derived write intention missing");
        };
        let actual = claim.budget();
        ensure!(
            actual.max_bytes <= budget.max_bytes.get()
                && actual.max_files <= budget.max_files.get()
                && actual.max_memory_bytes <= budget.max_memory_bytes.get()
                && actual.max_millis <= budget.max_millis.get(),
            "derived dispatch exceeds approved budget"
        );
        let recipe = self
            .derived
            .take()
            .context("derived recipe already consumed")?;
        let work = Work::split_claim(
            claim,
            processing.key(),
            Reservation {
                memory_bytes: PUBLICATION_MEMORY,
                temporary_bytes: self
                    .maximum
                    .checked_add(8192)
                    .context("derived output overflow")?,
            },
        )?;
        let durable = Vec::with_capacity(self.names.len());
        Ok((
            Publication {
                durable,
                plan: self,
                stage: None,
                phase: Phase::Prepared,
                accounted: 0,
                total: 0,
                index: 0,
                work,
            },
            recipe,
        ))
    }
}
impl NativeProcessing {
    /// Extract with actual Process scope and a prepaid separately qualified publication destination.
    pub fn extract_planned(
        self,
        claim: Claim,
        plan: NativePublicationPlan,
    ) -> std::result::Result<NativeAdmitted<Extraction>, NativeAdmittedError<Extraction>> {
        self.run(
            claim,
            Inputs::default(),
            move |processing, claim, inputs| {
                let (publication, recipe) = plan.start_derived(processing, claim)?;
                inputs.publication = Some(publication);
                let NativeDerivedRecipe::Text {
                    max_bytes,
                    child_rss_bytes,
                } = recipe
                else {
                    anyhow::bail!("actual text plan required");
                };
                knowledge_processing::extract::extract_file_funded(
                    processing.source().file()?,
                    processing.source().hint(),
                    max_bytes,
                    child_rss_bytes,
                    &mut processing.funding(claim)?,
                )
            },
        )
    }
    /// Segment under the exact native Media recipe with all publication siblings prepaid before M3.
    pub fn segment_video_planned(
        self,
        claim: Claim,
        plan: NativePublicationPlan,
    ) -> std::result::Result<NativeAdmitted<HeldVideo>, NativeAdmittedError<HeldVideo>> {
        self.run(
            claim,
            Inputs::default(),
            move |processing, claim, inputs| {
                let (publication, recipe) = plan.start_derived(processing, claim)?;
                inputs.publication = Some(publication);
                let NativeDerivedRecipe::Video(plan) = recipe else {
                    anyhow::bail!("actual video plan required");
                };
                crate::native_service::held::encode_video(processing, claim, inputs, plan)
            },
        )
    }
}
impl NativeAdmitted<Extraction> {
    /// Publish the paid extracted UTF-8 bytes without another complete buffer or file.
    pub fn publish(mut self) -> std::result::Result<Self, NativeAdmittedError<Extraction>> {
        let result = (|| {
            self.value()?;
            let processing = self
                .processing
                .as_ref()
                .context("native processing owner")?;
            let claim = self.claim.as_ref().context("native Claim")?;
            let value = &self.output.as_ref().context("actual text")?.value;
            ensure!(
                matches!(value.status.as_str(), "extracted" | "partial"),
                "no actual extracted representation to publish"
            );
            self.inputs
                .publication
                .as_mut()
                .context("actual publication plan required")?
                .publish_input(Output::Bytes(value.text.as_bytes()), || {
                    processing.check(claim)
                })
        })();
        match result {
            Ok(()) => Ok(self),
            Err(cause) => Err(NativeAdmittedError {
                cause,
                owner: Box::new(self),
            }),
        }
    }
}
impl NativeAdmitted<HeldVideo> {
    /// Install each explicit final name in source-clock order, preserving earlier durable clips on failure.
    pub fn publish(mut self) -> std::result::Result<Self, NativeAdmittedError<HeldVideo>> {
        let result = (|| {
            self.value()?;
            let processing = self
                .processing
                .as_ref()
                .context("native processing owner")?;
            let claim = self.claim.as_ref().context("native Claim")?;
            let value = &self.output.as_ref().context("actual video")?.value;
            let publication = self
                .inputs
                .publication
                .as_mut()
                .context("actual publication plan required")?;
            ensure!(
                value.segments.len() == publication.plan.names.len(),
                "actual clip count differs from explicit native destination plan"
            );
            for segment in &value.segments {
                publication.publish(&segment.artifact.file, || processing.check(claim))?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(self),
            Err(cause) => Err(NativeAdmittedError {
                cause,
                owner: Box::new(self),
            }),
        }
    }
}
