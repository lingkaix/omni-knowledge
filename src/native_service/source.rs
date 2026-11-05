//! Native read and queued-processing carrier. IDs/digests are data, never authority.
use super::*;
use knowledge_core::admission::Account;
use knowledge_core::{
    backend::Budget,
    data::ContentDigest,
    jobs::{Claim, Key, Priority, Submission, Submitted},
};
use knowledge_processing::funding::Funding;

/// An actual selected source descriptor, policy/version guard and prepaid native delivery lifetime.
/// Keep this carrier (or its Delivery) until the last controllable output write settles.
pub struct NativeSource {
    file: File,
    library: wire::Id,
    path: wire::RelativePath,
    context: String,
    source: String,
    version: ContentDigest,
    permit: Permit,
}
impl NativeSource {
    /// Validate this concrete processing effect against the actual sealed admitted request,
    /// never a status/browse permit, caller boolean or naked source ID.
    pub(super) fn indexed_extract_scope(&self, claim: &Claim, max_bytes: usize) -> Result<()> {
        self.check()?;
        let wire::Command::Process {
            library_id,
            source_ids,
            recipe,
            budget,
            ..
        } = self.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("native index input requires the actual Process command");
        };
        anyhow::ensure!(
            library_id == &self.library
                && matches!(
                    *recipe,
                    wire::ProcessRecipe::Extract | wire::ProcessRecipe::Embed
                )
                && source_ids
                    .iter()
                    .any(|id| id.as_str().strip_prefix("source-") == Some(self.source.as_str())),
            "native processing source/recipe differs from admitted command"
        );
        let actual = claim.budget();
        anyhow::ensure!(
            max_bytes as u64 <= budget.max_bytes.get()
                && actual.max_bytes <= budget.max_bytes.get()
                && actual.max_files <= budget.max_files.get()
                && actual.max_millis <= budget.max_millis.get()
                && actual.max_memory_bytes <= budget.max_memory_bytes.get(),
            "native processing exceeds the actual admitted request budget"
        );
        Ok(())
    }
    pub(super) fn embedding_scope(&self, claim: &Claim) -> Result<()> {
        self.indexed_extract_scope(claim, 0)?;
        anyhow::ensure!(
            matches!(
                self.permit.admitted_command().map_err(refused_error)?,
                wire::Command::Process {
                    recipe: wire::ProcessRecipe::Embed,
                    ..
                }
            ),
            "actual Process/Embed intention required before transmitting source text"
        );
        Ok(())
    }
    pub(super) fn check_model_selection(
        &self,
        lease: &omni_service::authority::SelectionLease,
    ) -> Result<()> {
        self.check()?;
        lease.check_permit(&self.permit).map_err(refused_error)?;
        Ok(())
    }
    pub(super) fn open(
        library: &wire::Id,
        path: &wire::RelativePath,
        permit: &Permit,
        budget: Budget,
    ) -> Result<Self> {
        if budget.max_files == 0 || budget.max_memory_bytes < 256 * 1024 || budget.max_millis == 0 {
            bail!("native verification budget exhausted");
        }
        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(budget.max_millis))
            .context("verification deadline overflow")?;
        permit.check_path(library, path).map_err(refused_error)?;
        permit
            .reserve_result(128 * 1024 + (library.as_str().len() + path.as_str().len()) as u64 * 16)
            .map_err(refused_error)?;
        let file = permit
            .with_root(library, |root| LibraryHost::open_root(root, path.as_str()))
            .map_err(refused_error)?;
        let before = file.metadata()?;
        if before.len() > budget.max_bytes {
            bail!("native verification byte budget exhausted");
        }
        if !before.is_file() {
            bail!("native source requires a regular file");
        }
        LibraryHost::track_version(library, path.as_str(), &file, &before, permit)
            .map_err(refused_error)?;
        let context = permit
            .with_selected_context(library, |root, policy, revision| {
                use std::fmt::Write;
                struct Sink(Sha256);
                impl std::fmt::Write for Sink {
                    fn write_str(&mut self, s: &str) -> std::fmt::Result {
                        self.0.update(s.as_bytes());
                        Ok(())
                    }
                }
                let info = root.metadata().map_err(refused)?;
                let mut sink = Sink(Sha256::new());
                write!(
                    &mut sink,
                    "native-read-v1:{}:{}:{}:{policy:?}",
                    info.dev(),
                    info.ino(),
                    revision
                )
                .map_err(|_| wire::ErrorCode::Internal)?;
                sink.0.update(current_marker(root)?);
                Ok(hex::encode(sink.0.finalize()))
            })
            .map_err(refused_error)?;
        // read_at keeps the original FD position unchanged; every controlled refill checks policy,
        // root namespace, the held version AND its current descriptor-relative name.
        use std::os::unix::fs::FileExt;
        let mut hash = Sha256::new();
        let mut offset = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if std::time::Instant::now() >= deadline {
                bail!("native verification deadline elapsed");
            }
            permit.check_path(library, path).map_err(refused_error)?;
            let count = file.read_at(&mut buffer, offset)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            offset = offset
                .checked_add(count as u64)
                .context("source offset overflow")?;
            if offset > before.len() {
                bail!("native source changed during verification");
            }
        }
        permit.check().map_err(refused_error)?;
        if offset != before.len() {
            bail!("native source changed during verification");
        }
        let version = ContentDigest::new(hex::encode(hash.finalize()))?;
        Ok(Self {
            file,
            library: library.clone(),
            path: path.clone(),
            context,
            source: hex::encode(Sha256::digest(path.as_str().as_bytes())),
            version,
            permit: permit.clone(),
        })
    }
    /// Recheck the actual current namespace, selected read policy and held/current source version.
    pub fn check(&self) -> Result<()> {
        self.permit
            .check_path(&self.library, &self.path)
            .map_err(refused_error)
    }
    /// Borrow this exact verified FD, never resolve a hint and reopen it. Check again after work
    /// and before each controllable delivery; a descriptor alone is not ongoing read permission.
    pub fn file(&self) -> Result<&File> {
        self.check()?;
        Ok(&self.file)
    }
    /// MIME/extension hint only; never reopen it or use it as authority.
    pub fn hint(&self) -> &Path {
        Path::new(self.path.as_str())
    }
    /// Actual native selected library label, not a caller-supplied constant.
    pub fn library_id(&self) -> &wire::Id {
        &self.library
    }
    /// Actual current root identity/policy/revision/marker binding, not an authority token.
    pub fn context(&self) -> &str {
        &self.context
    }
    /// Complete digest read from the actual held source, with current-version checks.
    pub fn version(&self) -> &ContentDigest {
        &self.version
    }
    /// Borrow the same native selected root/authority carrier for qualified owner adapters.
    pub fn permit(&self) -> &Permit {
        &self.permit
    }
    /// Admit the COMPLETE simultaneous response/escaping/deep-copy peak BEFORE allocation.
    pub fn reserve_result(&self, bytes: u64) -> Result<()> {
        self.permit.reserve_result(bytes).map_err(refused_error)
    }
    /// Keep actual native source pins and charges in the existing Delivery, through real write
    /// settlement. The response and encoder peak must already have been prepaid.
    pub fn into_delivery(
        self,
        response: wire::Response,
    ) -> std::result::Result<Delivery, wire::ErrorCode> {
        Delivery::from_native(response, self.permit)
    }
    /// Retain an independently admitted member's actual version pin in this SAME final delivery scope.
    /// This adds native FD/name checks, not a recursive Permit callback or path-derived authority.
    pub(super) fn retain_member(&self, member: &NativeSource) -> Result<()> {
        self.check()?;
        member.check()?;
        if self.library != member.library || self.context != member.context {
            bail!("native publication members require the same current selected context");
        }
        self.permit
            .check_path(&self.library, &member.path)
            .map_err(refused_error)?;
        let pin_bytes = (member.path.as_str().len() as u64 + member.library.as_str().len() as u64)
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| anyhow::anyhow!("native member pin overflow"))?;
        self.reserve_result(pin_bytes)?;
        LibraryHost::track_version(
            &self.library,
            member.path.as_str(),
            &member.file,
            &member.file.metadata()?,
            &self.permit,
        )
        .map_err(refused_error)
    }
    /// Bind native source facts to the owner's actual recipe and processor digests. Those two
    /// digests describe real work/code; neither them nor the resulting key grants authority.
    pub fn processing(
        self,
        recipe: ContentDigest,
        processor: ContentDigest,
    ) -> Result<NativeProcessing> {
        self.check()?;
        self.reserve_result(2048)?;
        let mut fingerprint = Sha256::new();
        fingerprint.update(admitted::processor_identity().as_bytes());
        fingerprint.update(include_bytes!("source.rs"));
        fingerprint.update(include_bytes!("extraction.rs"));
        fingerprint.update(include_bytes!("held.rs"));
        fingerprint.update(include_bytes!("models.rs"));
        fingerprint.update(include_bytes!("../../crates/omni-service/src/authority.rs"));
        fingerprint.update(include_bytes!("../../crates/omni-service/src/runtime.rs"));
        fingerprint.update(include_bytes!(
            "../../crates/knowledge-processing/src/held.rs"
        ));
        fingerprint.update(include_bytes!("../lib.rs"));
        fingerprint.update(include_bytes!("../../Cargo.lock"));
        fingerprint.update(processor.as_str().as_bytes());
        let processor = ContentDigest::new(hex::encode(fingerprint.finalize()))?;
        Ok(NativeProcessing {
            source: self,
            recipe,
            processor,
        })
    }
}
pub(super) fn refused_error(code: wire::ErrorCode) -> anyhow::Error {
    anyhow::anyhow!("native source refused: {code:?}")
}

/// Real queued-work key bound to its selected source, not another executor/account or numeric grant.
/// Queued jobs must retain this owner. A returned funding view does not itself allocate anything.
pub struct NativeProcessing {
    recipe: ContentDigest,
    processor: ContentDigest,
    // Source owns the native context/FD and aggregate holding: drop it AFTER all paid key fields.
    source: NativeSource,
}
/// Resource/intent data for the existing Core scheduler, never a grant or another queue.
pub struct NativeJobPlan<'a> {
    /// Existing operation receipt label.
    pub operation_id: &'a str,
    /// Existing job receipt label.
    pub job_id: &'a str,
    /// Complete concurrent work bounds, including caller-owned envelopes.
    pub budget: Budget,
    /// Actual simultaneous resource need; Core distinguishes TooLarge from queued contention.
    pub reservation: Reservation,
    /// Existing scheduler priority.
    pub priority: Priority,
    /// Existing retry bound.
    pub max_attempts: u64,
}
impl NativeProcessing {
    /// Borrow the same actual FD/current-policy carrier through execution and delivery.
    pub fn source(&self) -> &NativeSource {
        &self.source
    }
    /// Actual complete queue key, with native root context/library/path identity and source digest.
    pub fn key(&self) -> Key<'_> {
        Key {
            context: &self.source.context,
            library: self.source.library.as_str(),
            mount: None,
            source: &self.source.source,
            version: &self.source.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }
    /// Submit a visible real job EVEN WHEN another job is queued/running. Core preserves the
    /// impossible-need/contention distinction; this never invents a pending/waiting grant/string.
    pub fn submit(&self, plan: NativeJobPlan<'_>) -> Result<Submitted> {
        self.source.check()?;
        Ok(Account::current()?.submit(Submission {
            operation_id: plan.operation_id,
            job_id: plan.job_id,
            key: self.key(),
            budget: plan.budget,
            reservation: plan.reservation,
            priority: plan.priority,
            max_attempts: plan.max_attempts,
        })?)
    }
    /// Move the same source/delivery carrier out only after actual executor settlement. Unknown
    /// work must retain this entire bridge, the actual Claim and M3 unsettled capabilities instead.
    pub fn into_source(self) -> NativeSource {
        self.source
    }
    /// Use only an actual scheduler-returned Claim. M1 validates its exact generation/key and M3
    /// spends only via native split_claim; no independent debit or metadata-as-permission fallback.
    pub fn funding<'a>(&'a self, claim: &'a mut Claim) -> Result<Funding<'a>> {
        self.source.check()?;
        if let Some(stop) = claim.stop_requested(self.key())? {
            return Err(admitted::Stopped(stop).into());
        }
        let funding = Funding::Claim {
            claim,
            expected: self.key(),
        };
        funding.available()?;
        Ok(funding)
    }
    /// Recheck native source/policy and exact current claim before a controlled executor boundary.
    pub fn check(&self, claim: &Claim) -> Result<()> {
        self.source.check()?;
        if let Some(stop) = claim.stop_requested(self.key())? {
            return Err(admitted::Stopped(stop).into());
        }
        Ok(())
    }
}
