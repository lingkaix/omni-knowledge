//! Per-call prepaid effects on Core's sole account; this is not a queue or a second pool.

use std::fs::File;
use std::io;

use knowledge_core::admission::{Decision, Holding, Origin, Waiting};
use knowledge_core::jobs::{Claim, Key, Reservation};

use crate::admission::{self, AdmitError, Settled};

/// Origin of the actual processing credits. A funded call transfers existing claim credit.
/// The mutable claim borrow serializes spends made through this adapter. No digest is constructed here.
pub enum Funding<'a> {
    /// Native source inspection before verified job identity exists.
    BeforeVerification,
    /// Exact current claim and facts supplied by the native job owner.
    Claim {
        /// Existing account-owned execution capability, borrowed exclusively for this call.
        claim: &'a mut Claim,
        /// The same verified work facts already admitted by Core.
        expected: Key<'a>,
    },
}

impl Funding<'_> {
    /// Poll the native claim state and its real deadline during every watchdog/I/O tick.
    pub(crate) fn checkpoint(&self) -> anyhow::Result<()> {
        if let Self::Claim { claim, expected } = self {
            if std::time::Instant::now() >= claim.deadline() {
                anyhow::bail!(crate::worker::WorkerFailure::Deadline);
            }
            if claim.stop_requested(*expected)?.is_some() {
                anyhow::bail!(crate::worker::WorkerFailure::Cancelled);
            }
        }
        Ok(())
    }

    /// Remaining spendable credit; a job uses its existing execution funds, not global availability.
    pub fn available(&self) -> Result<Reservation, AdmitError> {
        let account = admission::current()?;
        match self {
            Self::BeforeVerification => {
                let status = account.status().map_err(admission::map_error)?;
                Ok(Reservation {
                    memory_bytes: status.memory_available,
                    temporary_bytes: status.storage_available,
                })
            }
            Self::Claim { claim, expected } => Ok(account
                .job_status(claim.job(), *expected)
                .map_err(admission::map_error)?
                .execution),
        }
    }

    /// Check the entire concurrent call before taking any component or causing processing effects.
    fn check(&self, need: Reservation) -> Result<(), AdmitError> {
        let policy = admission::current()?.policy();
        if need.memory_bytes > policy.memory_bytes {
            return Err(AdmitError::TooLargeMemory);
        }
        if need.temporary_bytes > policy.storage_bytes {
            return Err(AdmitError::TooLargeStorage);
        }
        let available = self.available()?;
        if need.memory_bytes > available.memory_bytes
            || need.temporary_bytes > available.temporary_bytes
        {
            return Err(match self {
                Self::Claim { .. } => AdmitError::InsufficientCredit,
                Self::BeforeVerification if need.memory_bytes > available.memory_bytes => {
                    AdmitError::WaitingMemory
                }
                Self::BeforeVerification => AdmitError::WaitingStorage,
            });
        }
        Ok(())
    }

    /// Acquire a component before effects; Core's existing transfer never reserves it twice.
    fn component(&mut self, need: Reservation) -> Result<Settled, AdmitError> {
        if need.memory_bytes == 0 && need.temporary_bytes == 0 {
            return Ok(Settled(None));
        }
        let account = admission::current()?;
        let holding = match self {
            Self::Claim { claim, expected } => account
                .split_claim(claim, *expected, need)
                .map_err(admission::map_error)?,
            Self::BeforeVerification => match account
                .reserve(need, Origin::BeforeVerification)
                .map_err(admission::map_error)?
            {
                Decision::Ready(holding) => holding,
                Decision::Waiting(Waiting::Memory) => return Err(AdmitError::WaitingMemory),
                Decision::Waiting(Waiting::Storage) => return Err(AdmitError::WaitingStorage),
            },
        };
        Ok(Settled(Some(holding)))
    }
}

/// A settled writer whose actual native write could not be reconciled still owns its real file and promise.
/// The host retains this error and its cache lease; dropping it never finishes Core's holding.
pub struct UnsettledWrite {
    /// Actual generated inode, not a later pathname reopen.
    file: File,
    /// Unreconciled capability from the same process account.
    holding: Option<Holding>,
    /// Whether the real delta already succeeded, so a finish retry cannot replay it.
    reconciled: bool,
    /// Actual native or account failure, retained for inspection.
    cause: anyhow::Error,
    /// Owned output/staging paths that must remain inspectable until reconciliation.
    pub artifacts: Vec<std::path::PathBuf>,
    /// Source, directory, and other generated descriptors retained by a held-FD call.
    pub(crate) leases: Vec<File>,
    /// Live native output records remain charged until their owner destroys them.
    pub(crate) retained: Vec<Holding>,
}
impl std::fmt::Debug for UnsettledWrite {
    /// Do not expose file contents or raw source paths in automatic diagnostics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnsettledWrite")
            .field("reconciled", &self.reconciled)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for UnsettledWrite {
    /// A fixed incomplete classification is not a saved/completed result.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("generated write reconciliation is unresolved; native file and charge retained")
    }
}
impl std::error::Error for UnsettledWrite {
    /// Preserve the actual cause without manufacturing a completion receipt.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl UnsettledWrite {
    /// Inspect the actual owned descriptor while the caller retains its native cache lease.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Inspect source/directory/output leases without reopening any path.
    pub fn leases(&self) -> &[File] {
        &self.leases
    }

    /// Transfer record credit after reconciliation; destroy native records before finishing these holdings.
    /// Returns `Stale` until the actual extent has been reconciled and its storage holding finished.
    pub fn take_retained(&mut self) -> Result<Vec<Holding>, AdmitError> {
        if self.holding.is_some() {
            return Err(AdmitError::Stale);
        }
        Ok(std::mem::take(&mut self.retained))
    }

    /// Retry only reconciliation for an already stopped writer; each confirmed delta is debited at most once.
    pub fn retry(&mut self) -> anyhow::Result<()> {
        let holding = self.holding.as_mut().ok_or(AdmitError::Stale)?;
        if !self.reconciled {
            holding.written(self.file.metadata()?.len())?;
            self.reconciled = true;
        }
        holding.finish()?;
        self.holding.take();
        Ok(())
    }
}

/// Reconcile one new inode after actual process/I/O settlement, retaining unknown accounting instead of RAII release.
pub(crate) fn reconcile_file(
    file: File,
    storage: &mut Settled,
    artifacts: &[&std::path::Path],
) -> anyhow::Result<(u64, File)> {
    let result = file
        .metadata()
        .map(|m| m.len())
        .map_err(anyhow::Error::from)
        .and_then(|length| {
            storage
                .written(length)
                .map(|_| length)
                .map_err(anyhow::Error::from)
        });
    match result {
        Ok(length) => Ok((length, file)),
        Err(cause) => {
            let holding = storage
                .0
                .take()
                .ok_or_else(|| anyhow::anyhow!("Generated writer has no storage capability"))?;
            Err(anyhow::Error::new(UnsettledWrite {
                file,
                holding: Some(holding),
                reconciled: false,
                cause,
                artifacts: artifacts.iter().map(|p| p.to_path_buf()).collect(),
                leases: Vec::new(),
                retained: Vec::new(),
            }))
        }
    }
}

/// Captured diagnostics retain the existing one-MiB ceiling, but are now memory, not uncharged files.
pub(crate) const DIAGNOSTIC_CAP: u64 = 1024 * 1024;

/// One complete call's distinct lifetimes, all funded before the first read/allocation/child/write.
/// Construction rolls back pre-effect components on refusal; no waiting call leaves a partial charge.
pub(crate) struct Credits {
    /// Input buffer; declared before buffers so callers drop the buffers first.
    pub input: Settled,
    /// Returned data allowance, acquired before parsing/copying it.
    pub output: Settled,
    /// Concurrent capture/JSON serialization buffers, including diagnostic storage in memory.
    pub transport: Settled,
    /// All processes in this call's one owned group, retained on unknown collection.
    pub child: Settled,
    /// Actual generated writes on the selected volume, retained if their writer is unresolved.
    pub storage: Settled,
    /// Promised group resident memory; this is independent of the input/result/transport memory.
    pub rss: u64,
}

impl Credits {
    /// Admit a complete peak, with optional child memory drawn only from the credit left after buffers.
    pub fn new(
        funding: &mut Funding<'_>,
        input: u64,
        output: u64,
        transport: u64,
        child: Option<u64>,
        storage: u64,
    ) -> Result<Self, AdmitError> {
        let fixed = input
            .checked_add(output)
            .and_then(|n| n.checked_add(transport))
            .ok_or(AdmitError::TooLargeMemory)?;
        let rss = match child {
            Some(bytes) => bytes,
            None => funding.available()?.memory_bytes.saturating_sub(fixed),
        };
        // Every capture plan here runs a worker. Zero means no worker can start, not a late wait after reading input.
        if transport > 0 && rss == 0 {
            if fixed.checked_add(1).ok_or(AdmitError::TooLargeMemory)?
                > admission::current()?.policy().memory_bytes
            {
                return Err(AdmitError::TooLargeMemory);
            }
            return Err(match funding {
                Funding::Claim { .. } => AdmitError::InsufficientCredit,
                Funding::BeforeVerification => AdmitError::WaitingMemory,
            });
        }
        let memory = fixed.checked_add(rss).ok_or(AdmitError::TooLargeMemory)?;
        funding.check(Reservation {
            memory_bytes: memory,
            temporary_bytes: storage,
        })?;
        let mut memory_part = |bytes| {
            funding.component(Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            })
        };
        let input = memory_part(input)?;
        let output = memory_part(output)?;
        let transport = memory_part(transport)?;
        let child = memory_part(rss)?;
        let storage = funding.component(Reservation {
            memory_bytes: 0,
            temporary_bytes: storage,
        })?;
        Ok(Self {
            input,
            output,
            transport,
            child,
            storage,
            rss,
        })
    }

    /// Move only the returned-data credit to its owner; all dead working buffers settle separately.
    pub fn retained(&mut self) -> Vec<Holding> {
        self.output.0.take().into_iter().collect()
    }
}

/// Bound the descriptor read itself, including a growing inode. The cursor is not shared or changed.
/// `capacity` was prepaid; no `read_to_end` can grow the allocation beyond it.
pub(crate) fn read_fd(file: &File, capacity: usize) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        let mut bytes = vec![0; capacity];
        let mut filled = 0;
        while filled < capacity {
            match file.read_at(&mut bytes[filled..], filled as u64) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        bytes.truncate(filled);
        Ok(bytes)
    }
    #[cfg(not(unix))]
    {
        let _ = (file, capacity);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bounded descriptor read is unavailable",
        ))
    }
}

/// Require the actual generated destination descriptor to be on the account's selected volume.
pub(crate) fn selected_volume(file: &File) -> Result<(), AdmitError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let device = file.metadata().map_err(|_| AdmitError::Account)?.dev();
        if device != admission::current()?.policy().startup_volume.device {
            return Err(AdmitError::Account);
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(AdmitError::Account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Actual native reconciliation failure keeps the file/capability; an explicit credit repair retries accounting, never the writer.
    #[test]
    fn unreconciled_native_extent_keeps_cost_and_does_not_replay_delta() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"ab").unwrap(); // Owned fault fixture; real processor write gating is tested with FFmpeg.
        let before = admission::current().unwrap().status().unwrap().reserved;
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(&mut funding, 0, 0, 0, Some(0), 1).unwrap();
        let mut error = reconcile_file(file, &mut credits.storage, &[])
            .expect_err("actual extent exceeds supplied reconciliation credit");
        assert!(
            credits.storage.0.is_none(),
            "unknown costs leave the RAII release path"
        );
        let pending = error.downcast_mut::<UnsettledWrite>().unwrap();
        assert_eq!(pending.file().metadata().unwrap().len(), 2);
        assert_eq!(
            admission::current()
                .unwrap()
                .status()
                .unwrap()
                .reserved
                .temporary_bytes,
            before.temporary_bytes + 1
        );
        assert!(matches!(
            pending
                .holding
                .as_mut()
                .unwrap()
                .grow(Reservation {
                    memory_bytes: 0,
                    temporary_bytes: 1
                })
                .unwrap(),
            Decision::Ready(())
        ));
        pending.retry().unwrap();
        assert_eq!(
            admission::current().unwrap().status().unwrap().reserved,
            before
        );
        assert!(
            pending.retry().is_err(),
            "finished reconciliation cannot debit the same actual extent twice"
        );
    }

    /// A real admitted inode can grow after inspection; the read and allocation must still stop at the prepaid boundary.
    #[test]
    fn grown_descriptor_read_never_grows_the_prepaid_buffer() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let mut source = tempfile::tempfile().unwrap();
        source.write_all(b"original").unwrap();
        let observed = source.metadata().unwrap().len();
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(&mut funding, observed + 1, 0, 0, Some(0), 0).unwrap();
        source.set_len(8 * 1024 * 1024).unwrap();
        let bytes = read_fd(&source, observed as usize + 1).unwrap();
        assert_eq!(source.metadata().unwrap().len(), 8 * 1024 * 1024);
        assert_eq!(bytes.len(), observed as usize + 1);
        assert_eq!(bytes.capacity(), observed as usize + 1);
        assert_eq!(&bytes[..observed as usize], b"original");
        assert!(
            credits
                .input
                .0
                .as_ref()
                .unwrap()
                .remaining()
                .unwrap()
                .memory_bytes
                >= bytes.capacity() as u64
        );
        drop(bytes);
        credits.input.release_now().unwrap();
    }
}
