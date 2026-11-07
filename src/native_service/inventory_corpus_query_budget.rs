//! Data ceilings within ACTUAL already prepaid scope/Claim resources.
//! No Core account/queue/promise/refund/source grant or permission cache is constructed here.
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
/// Scalar DATA only: admitted bytes and source/metadata object examinations, not distinct hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_service) struct DataAmount {
    pub bytes: u64,
    pub objects: u64,
}
/// Counts conservative admitted work, not measured RSS or invented completed IO.
/// Repeated source/metadata examinations are work; unavailable entries are not free objects.
/// Failed opaque calls keep their possible span. This is not a second Core resource account.
pub(in crate::native_service) struct ReadAllowance {
    used: AtomicU64,
    objects: AtomicU64,
    denied: AtomicBool,
    io: std::sync::Mutex<()>,
}
impl ReadAllowance {
    pub(in crate::native_service) fn new(used: u64) -> Self {
        Self::from_amount(DataAmount {
            bytes: used,
            objects: 0,
        })
    }
    pub(super) fn from_amount(amount: DataAmount) -> Self {
        Self {
            used: AtomicU64::new(amount.bytes),
            objects: AtomicU64::new(amount.objects),
            denied: AtomicBool::new(false),
            io: std::sync::Mutex::new(()),
        }
    }
    pub(in crate::native_service) fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }
    pub(in crate::native_service) fn amount(&self) -> DataAmount {
        DataAmount {
            bytes: self.used(),
            objects: self.objects.load(Ordering::Relaxed),
        }
    }
    pub(in crate::native_service) fn check(&self) -> Result<()> {
        if self.denied.load(Ordering::Relaxed) {
            Err(QueryBudgetExhausted.into())
        } else {
            Ok(())
        }
    }
    fn add(&self, counter: &AtomicU64, amount: u64, limit: u64) -> Result<()> {
        self.check()?;
        if counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                n.checked_add(amount).filter(|n| *n <= limit)
            })
            .is_err()
        {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        Ok(())
    }
    pub(in crate::native_service) fn admit(&self, bytes: u64, limit: u64) -> Result<()> {
        let _io = self
            .io
            .lock()
            .map_err(|_| anyhow::anyhow!("actual Query read allowance poisoned"))?;
        self.add(&self.used, bytes, limit)
    }
    pub(in crate::native_service) fn admit_object(&self, limit: u64) -> Result<()> {
        let _io = self
            .io
            .lock()
            .map_err(|_| anyhow::anyhow!("actual Query read allowance poisoned"))?;
        self.add(&self.objects, 1, limit)
    }
    /// Exclude competing admissions while the existing synchronous M1 decoder owns its complete
    /// possible byte/object span. Only known unconsumed DATA returns; Core holdings never change.
    pub(super) fn with_remaining<T>(
        &self,
        limit: DataAmount,
        call: impl FnOnce(DataAmount) -> Result<(T, DataAmount)>,
    ) -> Result<T> {
        let _io = self
            .io
            .lock()
            .map_err(|_| anyhow::anyhow!("actual Query read allowance poisoned"))?;
        self.check()?;
        let before = self.amount();
        let left = DataAmount {
            bytes: limit
                .bytes
                .checked_sub(before.bytes)
                .ok_or(QueryBudgetExhausted)?,
            objects: limit
                .objects
                .checked_sub(before.objects)
                .ok_or(QueryBudgetExhausted)?,
        };
        if left.bytes == 0 || left.objects == 0 {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        self.used.store(limit.bytes, Ordering::Relaxed);
        self.objects.store(limit.objects, Ordering::Relaxed);
        let (value, spent) = call(left)?;
        if spent.bytes > left.bytes || spent.objects > left.objects {
            self.denied.store(true, Ordering::Relaxed);
            return Err(QueryBudgetExhausted.into());
        }
        self.used
            .store(before.bytes + spent.bytes, Ordering::Relaxed);
        self.objects
            .store(before.objects + spent.objects, Ordering::Relaxed);
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
    fn object_examinations_including_unavailable_are_terminal_work() {
        let budget = ReadAllowance::new(0);
        budget.admit_object(1).unwrap();
        assert_eq!(budget.amount().objects, 1);
        assert!(
            budget
                .admit_object(1)
                .unwrap_err()
                .is::<QueryBudgetExhausted>()
        );
        assert!(budget.admit(0, 1000).is_err());
    }
    #[test]
    fn opaque_sync_read_keeps_whole_span_until_known_cursor_returns() {
        let budget = ReadAllowance::from_amount(DataAmount {
            bytes: 7,
            objects: 2,
        });
        budget
            .with_remaining(
                DataAmount {
                    bytes: 100,
                    objects: 12,
                },
                |left| {
                    assert_eq!(
                        left,
                        DataAmount {
                            bytes: 93,
                            objects: 10
                        }
                    );
                    assert_eq!(
                        budget.amount(),
                        DataAmount {
                            bytes: 100,
                            objects: 12
                        }
                    );
                    Ok((
                        (),
                        DataAmount {
                            bytes: 13,
                            objects: 3,
                        },
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            budget.amount(),
            DataAmount {
                bytes: 20,
                objects: 5
            }
        );
        budget.admit(80, 100).unwrap();
        assert_eq!(budget.used(), 100);
        let failed = ReadAllowance::new(7);
        assert!(
            failed
                .with_remaining::<()>(
                    DataAmount {
                        bytes: 100,
                        objects: 12
                    },
                    |_| Err(anyhow::anyhow!("actual opaque read failed"))
                )
                .is_err()
        );
        assert_eq!(
            failed.amount(),
            DataAmount {
                bytes: 100,
                objects: 12
            }
        );
        assert!(failed.admit(1, 100).is_err());
    }
}
