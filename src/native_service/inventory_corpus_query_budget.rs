//! A data-IO ceiling within the ACTUAL already prepaid scope/Claim.
//! No Core account/queue/promise/refund/source grant or permission cache is constructed here.
//! Data spend is not Core credit: memory/FD/pins still retain their actual existing Work owners.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
/// Fixed typed refusal, never source absence/empty success or an admission grant.
#[derive(Debug)]
pub(in crate::native_service) struct QueryBudgetExhausted;
impl std::fmt::Display for QueryBudgetExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual Query data budget exhausted")
    }
}
impl std::error::Error for QueryBudgetExhausted {}
/// Counts admitted requested reads, not measured RSS or a fabricated completed-IO number.
/// Failed/partial reads remain conservatively spent. They are not retried as fresh work.
pub(in crate::native_service) struct ReadAllowance {
    used: AtomicU64,
    denied: AtomicBool,
    io: std::sync::Mutex<()>,
}
impl ReadAllowance {
    pub(in crate::native_service) fn new(used: u64) -> Self {
        Self {
            used: AtomicU64::new(used),
            denied: AtomicBool::new(false),
            io: std::sync::Mutex::new(()),
        }
    }
    pub(in crate::native_service) fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }
    pub(in crate::native_service) fn check(&self) -> Result<()> {
        if self.denied.load(Ordering::Relaxed) {
            Err(QueryBudgetExhausted.into())
        } else {
            Ok(())
        }
    }
    pub(in crate::native_service) fn admit(&self, bytes: u64, limit: u64) -> Result<()> {
        let _io = self
            .io
            .lock()
            .map_err(|_| anyhow::anyhow!("actual Query read allowance poisoned"))?;
        self.check()?;
        if self
            .used
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                n.checked_add(bytes).filter(|n| *n <= limit)
            })
            .is_err()
        {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        Ok(())
    }
    /// The existing synchronous M1 decoder has its own exact per-call read bound. Hold its WHOLE
    /// possible read span against this same counter before entering it, excluding parallel refills.
    /// Return only the unconsumed DATA span after a successful bounded decoder/known actual cursor;
    /// errors/panics keep the full conservative spend. This changes no Core holding or promise.
    pub(in crate::native_service) fn with_remaining<T>(
        &self,
        limit: u64,
        call: impl FnOnce(u64) -> Result<(T, u64)>,
    ) -> Result<T> {
        let _io = self
            .io
            .lock()
            .map_err(|_| anyhow::anyhow!("actual Query read allowance poisoned"))?;
        self.check()?;
        let before = self.used();
        let left = limit.checked_sub(before).ok_or(QueryBudgetExhausted)?;
        if left == 0 {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        self.used.store(limit, Ordering::Relaxed);
        let (value, spent) = call(left)?;
        if spent > left {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        self.used.store(before + spent, Ordering::Relaxed);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_read_ceiling_is_terminal_and_never_a_fresh_counter() {
        let budget = ReadAllowance::new(0);
        budget.admit(4, 4).unwrap();
        assert_eq!(budget.used(), 4);
        assert!(budget.admit(1, 4).unwrap_err().is::<QueryBudgetExhausted>());
        assert!(budget.admit(0, 1000).is_err());
    }
    #[test]
    fn opaque_sync_read_keeps_whole_span_until_known_cursor_returns() {
        let budget = ReadAllowance::new(7);
        budget
            .with_remaining(100, |left| {
                assert_eq!(left, 93);
                assert_eq!(budget.used(), 100);
                Ok(((), 13))
            })
            .unwrap();
        assert_eq!(budget.used(), 20);
        budget.admit(80, 100).unwrap();
        assert_eq!(budget.used(), 100);
        let failed = ReadAllowance::new(7);
        assert!(
            failed
                .with_remaining::<()>(100, |_| Err(anyhow::anyhow!("actual opaque read failed")))
                .is_err()
        );
        assert_eq!(failed.used(), 100);
        assert!(failed.admit(1, 100).is_err());
    }
}
