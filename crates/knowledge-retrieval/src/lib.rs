//! Actual shared-kernel invocations charged to the one native Core account.
//!
//! This adapter does not grant roots, invent digests, select source eligibility, perform file RRF,
//! or create another queue. Caller work and retained reader resources use the same process account.
#![deny(missing_docs)]

mod stored;
/// Package-owned verification ceilings for native callers, not admission or permission grants.
pub use knowledge_package::{Bounds as ArchiveBounds, Quotas as ArchiveQuotas};
pub use retrieval_kernel::{ByteDirectory, SourceGeneration, consumer_machinery};
pub use stored::{ArchiveSource, StoredGeneration, StoredIdentity, StoredPlan, StoredResult};

use knowledge_core::admission::{Account, Decision, Holding, Origin};
use knowledge_core::jobs::Reservation;
pub use retrieval_kernel::{
    Content, Control, Coverage, Document, Hit, Locator, Mode, Query, ScopeView, SearchResult,
    SourceKey,
};
use std::path::Path;

/// Fixed adapter refusal; kernel errors retain their typed code and bounded fixed diagnostics.
#[derive(Debug)]
pub enum Error {
    /// The actual Core account refused the reservation or is not started.
    Admission(knowledge_core::admission::Error),
    /// Caller has not reserved enough for this bounded kernel invocation.
    Reservation,
    /// Existing kernel admission/current-generation/processing refusal.
    Kernel(retrieval_kernel::Error),
    /// Checked accounting arithmetic or bounded inventory encoding failed.
    Inventory,
    /// Verified package omission, quota, waiting, drift or cancellation refusal.
    Package(knowledge_package::Error),
}
impl From<knowledge_package::Error> for Error {
    /// Keep optional-index omission distinct from source/admission failure.
    fn from(error: knowledge_package::Error) -> Self {
        Self::Package(error)
    }
}
impl std::fmt::Display for Error {
    /// Write classifications only, without source identities or paths.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "retrieval adapter: {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<knowledge_core::admission::Error> for Error {
    /// Preserve the shared native admission failure.
    fn from(error: knowledge_core::admission::Error) -> Self {
        Self::Admission(error)
    }
}
impl From<retrieval_kernel::Error> for Error {
    /// Preserve kernel coverage and generation classifications, not an empty successful result.
    fn from(error: retrieval_kernel::Error) -> Self {
        Self::Kernel(error)
    }
}

/// One noncloneable live resource lifetime. Dropping this value means its actual work has settled.
/// Split child/model/pin lifetimes before ending execution; never drop it while detached work lives.
pub struct Work(Holding);
impl Work {
    /// Reserve BEFORE caller/kernel allocation against the SAME Core account; no pool is constructed.
    /// A queued actual job supplies `Origin::Job`; pre-verification reads need no fabricated digest.
    pub fn reserve(reservation: Reservation, origin: Origin<'_>) -> Result<Decision<Self>, Error> {
        Ok(match Account::current()?.reserve(reservation, origin)? {
            Decision::Ready(holding) => Decision::Ready(Self(holding)),
            Decision::Waiting(reason) => Decision::Waiting(reason),
        })
    }
    /// Move actual claim resources into an independently retained lifetime without double charging.
    pub fn split_claim(
        claim: &knowledge_core::jobs::Claim,
        expected: knowledge_core::jobs::Key<'_>,
        reservation: Reservation,
    ) -> Result<Self, Error> {
        Ok(Self(Account::current()?.split_claim(
            claim,
            expected,
            reservation,
        )?))
    }
    /// Return a synchronous split loan to its exact original live claim without releasing aggregate
    /// charge. Never use this for detached work or retained values that no longer belong to that claim.
    pub fn rejoin(
        &mut self,
        claim: &knowledge_core::jobs::Claim,
        expected: knowledge_core::jobs::Key<'_>,
    ) -> Result<(), Error> {
        Ok(Account::current()?.rejoin_claim(&mut self.0, claim, expected)?)
    }
    /// Inspect this lifetime's actual remaining promise, not measured RSS.
    pub fn remaining(&self) -> Result<Reservation, Error> {
        Ok(self.0.remaining()?)
    }
    /// Grow before allocating/writing additional bytes; waiting acquires no partial charge.
    pub fn grow(&mut self, additional: Reservation) -> Result<Decision<()>, Error> {
        Ok(self.0.grow(additional)?)
    }
    /// Confirm an actual newly written generated-byte delta on the selected native cache volume.
    /// This removes only its unwritten promise; retained disk bytes remain in fresh native free-space facts.
    pub fn written(&mut self, bytes: u64) -> Result<knowledge_core::admission::Volume, Error> {
        Ok(self.0.written(bytes)?)
    }
    /// Check the concrete bounded invocation requirement before entering the actual kernel.
    fn require(&self, memory: u64, temporary: u64) -> Result<(), Error> {
        let held = self.remaining()?;
        if held.memory_bytes < memory || held.temporary_bytes < temporary {
            return Err(Error::Reservation);
        }
        Ok(())
    }
}
impl Drop for Work {
    /// Only synchronous settled resources are owned here; detached resources must retain their own Work.
    fn drop(&mut self) {
        let _ = self.0.finish();
    }
}

/// Native kernel facade, not a DTO backend or an alternate lexical engine.
pub struct KernelAdapter {
    kernel: retrieval_kernel::Kernel,
}
/// Actual native generation pin with an independently charged manifest/reader lifetime.
pub struct PinnedGeneration {
    // Declaration order is deliberate: end native reader use before releasing its memory charge.
    pin: retrieval_kernel::GenerationPin,
    _resources: Work,
}
impl PinnedGeneration {
    /// Exact immutable selected generation while the native reader lease is live.
    pub fn id(&self) -> &str {
        self.pin.id()
    }
}
impl KernelAdapter {
    /// Retain the actual process-global tokenizer/dictionary lifetime once on the same Core account.
    /// Hosts and generic byte/Directory adapters share this, never a second model registry or charge.
    pub fn initialize_runtime() -> Result<(), Error> {
        static RUNTIME: std::sync::OnceLock<Work> = std::sync::OnceLock::new();
        static START: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = START.lock().map_err(|_| Error::Reservation)?;
        if RUNTIME.get().is_none() {
            let Decision::Ready(work) = Work::reserve(
                Reservation {
                    memory_bytes: 256 * 1024 * 1024,
                    temporary_bytes: 0,
                },
                Origin::Startup,
            )?
            else {
                return Err(Error::Reservation);
            };
            let _ = RUNTIME.set(work);
        }
        Ok(())
    }
    /// Bind an actual kernel and consuming build fingerprint. No query/publication occurs here.
    /// The caller already owns source/cache selection; this method is not an authorization grant.
    pub fn open(source: &Path, store: &Path, build_identity: &str) -> Result<Self, Error> {
        Self::initialize_runtime()?;
        // Even native setup is a pre-verification lifetime in the one established account.
        let setup = match Work::reserve(
            Reservation {
                memory_bytes: 64 * 1024,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )? {
            Decision::Ready(work) => work,
            Decision::Waiting(_) => return Err(Error::Reservation),
        };
        let kernel = retrieval_kernel::Kernel::with_build_identity(source, store, build_identity)?;
        drop(setup);
        Ok(Self { kernel })
    }
    /// Conservative preflight requirement for a bounded shard, including caller inventory and writer.
    /// Inventory size is supplied from an already admitted streaming planner, not an arbitrary corpus cap.
    /// Existing prototype per-shard format ceilings remain unchanged; orchestration partitions inventories.
    pub fn publish_requirement(encoded_inventory_bytes: u64) -> Result<Reservation, Error> {
        Ok(Reservation {
            memory_bytes: encoded_inventory_bytes
                .checked_mul(16)
                .and_then(|v| v.checked_add(128 * 1024 * 1024))
                .ok_or(Error::Inventory)?,
            temporary_bytes: 512 * 1024 * 1024,
        })
    }
    /// Requirement for one bounded query/pin; persistent dictionary/runtime costs belong to the host.
    pub fn query_requirement() -> Reservation {
        Reservation {
            memory_bytes: 128 * 1024 * 1024,
            temporary_bytes: 0,
        }
    }
    /// Publish through the actual kernel after verifying the caller's admitted invocation lifetime.
    /// Cancellation/error does not turn an incomplete inventory into deletion authority.
    pub fn publish(
        &self,
        view: ScopeView,
        expected: Option<String>,
        work: &mut Work,
        control: &Control,
    ) -> Result<String, Error> {
        control.check()?;
        // The caller reserves before constructing the view. Encoding is bounded by the existing
        // shard format and used here only to verify that its supplied promise was not understated.
        work.require(128 * 1024 * 1024, 512 * 1024 * 1024)?;
        let bytes = serde_json::to_vec(&view).map_err(|_| Error::Inventory)?;
        let required = Self::publish_requirement(bytes.len() as u64)?;
        work.require(required.memory_bytes, required.temporary_bytes)?;
        Ok(self.kernel.publish_controlled(view, expected, control)?)
    }
    /// Query only the admitted scopes via the shared analyzer/scanner and actual native reader locks.
    pub fn search(
        &self,
        query: Query,
        work: &mut Work,
        control: &Control,
    ) -> Result<SearchResult, Error> {
        let required = Self::query_requirement();
        work.require(required.memory_bytes, required.temporary_bytes)?;
        Ok(self.kernel.search_controlled(query, control)?)
    }
    /// Hold a native reader pin. Its independent Work must outlive every use of this pin.
    pub fn pin(
        &self,
        scope: &str,
        work: Work,
        control: &Control,
    ) -> Result<PinnedGeneration, Error> {
        let required = Self::query_requirement();
        work.require(required.memory_bytes, 0)?;
        Ok(PinnedGeneration {
            pin: self.kernel.pin_generation(scope, control)?,
            _resources: work,
        })
    }
    /// Inspect only the exact named scope's current generation, without a global inventory.
    pub fn generation(&self, scope: &str) -> Result<Option<String>, Error> {
        Ok(self.kernel.generation(scope)?)
    }
}
