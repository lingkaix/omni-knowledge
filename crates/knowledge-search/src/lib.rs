//! File-rank orchestration and disk vectors using the ONE Core account and actual kernel facade.
//! No numeric grant constructor, pending admission, resource pool, or lexical engine lives here.
mod lexical;
mod partition_ranks;
mod rrf;
mod vectors;
use knowledge_core::{
    admission::{Decision, Origin},
    jobs::Reservation,
};
use knowledge_retrieval::Work;
pub use lexical::{
    AdmittedSource, LexicalError, LexicalHit, LexicalMode, LexicalOutcome, LexicalQuery,
    publish_lexical, query_lexical, search_lexical,
};
pub use partition_ranks::FileRankWindow;
pub use rrf::fuse_ranks;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
pub use vectors::{
    ModelIdentity, PreparedComponent, PreparedVectors, RepresentationIdentity, VectorBuildPlan,
    VectorCandidate, VectorError, VectorEvidence, VectorMetric, VectorModality,
    VectorNormalization, VectorResult, VectorRow, VectorStore, import_stored_segment,
};

/// A real non-forgeable Core lifetime. Clones retain the SAME promise, not independent spending credit.
/// The caller provides the complete concurrent operation envelope before constructing inputs.
#[derive(Clone)]
pub struct AccountGrant(Arc<Mutex<Option<Work>>>);
impl AccountGrant {
    pub fn from_work(work: Work) -> Self {
        Self(Arc::new(Mutex::new(Some(work))))
    }
    pub fn reserve(need: Reservation, origin: Origin<'_>) -> Result<Self, GrantError> {
        match Work::reserve(need, origin).map_err(|_| GrantError::Closed)? {
            Decision::Ready(work) => Ok(Self::from_work(work)),
            Decision::Waiting(_) => Err(GrantError::TooSmall),
        }
    }
    pub(crate) fn run<T>(
        &self,
        body: impl FnOnce(&mut Work) -> Result<T, GrantError>,
    ) -> Result<T, GrantError> {
        let mut work = self.0.lock().map_err(|_| GrantError::Closed)?;
        body(work.as_mut().ok_or(GrantError::Closed)?)
    }
    /// Move the one actual envelope into an affine long-lived reader. All clones
    /// observe the same consumed state; none can spend the reader's credit again.
    pub(crate) fn take(&self) -> Result<Work, GrantError> {
        self.0
            .lock()
            .map_err(|_| GrantError::Closed)?
            .take()
            .ok_or(GrantError::Closed)
    }
    pub(crate) fn covers(&self, memory: u64, temporary: u64) -> Result<(), GrantError> {
        self.run(|work| {
            let need = work.remaining().map_err(|_| GrantError::Closed)?;
            if need.memory_bytes < memory || need.temporary_bytes < temporary {
                Err(GrantError::TooSmall)
            } else {
                Ok(())
            }
        })
    }
}
impl std::fmt::Debug for AccountGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CoreResourceLifetime")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantError {
    Closed,
    TooSmall,
    Deadline,
    Kernel(String),
}
impl std::fmt::Display for GrantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Closed => "shared account unavailable",
            Self::TooSmall => "account promise is smaller than the concurrent operation",
            Self::Deadline => "retrieval deadline reached",
            Self::Kernel(_) => "actual kernel refused the operation",
        })
    }
}
impl std::error::Error for GrantError {}
pub(crate) fn before_work(
    grant: &AccountGrant,
    bytes: u64,
    deadline: Option<Instant>,
) -> Result<(), GrantError> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(GrantError::Deadline);
    }
    grant.covers(bytes, 0)
}
pub fn projection_bytes(sources: &[AdmittedSource]) -> u64 {
    sources.iter().fold(0u64, |sum, s| {
        sum.saturating_add(s.text.len() as u64)
            .saturating_add(s.path.len() as u64)
            .saturating_add(256)
    })
}
