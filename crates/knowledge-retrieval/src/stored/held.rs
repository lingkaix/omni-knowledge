//! Reuse the same currentness-checked range engine for actual immutable native component FDs.
//! This is not archive verification, a filesystem resolver, a new query engine or resource account.
use super::*;
/// One shared affine same-account range envelope. Consumers separately prepay their opaque costs.
/// All handles AND returned OwnedBytes retain this actual lifetime and source/pin, not just labels.
pub struct HeldRangeReaders {
    resources: Arc<Mutex<Work>>,
    control: Control,
    window: u64,
    reads: Arc<ReadLiveness>,
}
impl HeldRangeReaders {
    /// Existing range-reader concurrent buffer quote. No index/corpus size cap is introduced.
    pub fn requirement(window: u64) -> Result<knowledge_core::jobs::Reservation, Error> {
        if window == 0 || usize::try_from(window).is_err() {
            return Err(Error::Reservation);
        }
        Ok(knowledge_core::jobs::Reservation {
            memory_bytes: window
                .checked_mul(4)
                .and_then(|n| n.checked_add(64 * 1024))
                .ok_or(Error::Reservation)?,
            temporary_bytes: 0,
        })
    }
    /// Accept complete paid range peak before source inspection; sharing never reserves/refunds.
    pub fn new(work: Work, window: u64, control: Control) -> Result<Self, Error> {
        Self::new_owning(work, window, control).map_err(|(work, error)| {
            drop(work);
            error
        })
    }
    /// Validate a factory without losing an actual loan on refusal. The caller can rejoin or retry
    /// the SAME returned Work; neither expiry nor a bad quote releases/reserves global credit.
    pub fn new_owning(work: Work, window: u64, control: Control) -> Result<Self, (Work, Error)> {
        let checked = (|| -> Result<(), Error> {
            let need = Self::requirement(window)?;
            work.require(need.memory_bytes, 0)?;
            control.check()?;
            Ok(())
        })();
        if let Err(error) = checked {
            return Err((work, error));
        }
        Ok(Self {
            resources: Arc::new(Mutex::new(work)),
            control,
            window,
            reads: ReadLiveness::new(window * 4),
        })
    }
    /// Recover the EXACT affine Work only after every actual handle/OwnedBytes/FD owner dies.
    /// A live clone returns this actual owner; there is no finish/global-reserve gap. The caller
    /// may rejoin this same Work to its original Claim using the existing exclusive rejoin API.
    pub fn into_work(self) -> Result<Work, Self> {
        let Self {
            resources,
            control,
            window,
            reads,
        } = self;
        match Arc::try_unwrap(resources) {
            Ok(work) => {
                drop((control, reads));
                Ok(work
                    .into_inner()
                    .unwrap_or_else(std::sync::PoisonError::into_inner))
            }
            Err(resources) => Err(Self {
                resources,
                control,
                window,
                reads,
            }),
        }
    }
    /// Retain one actual admitted regular FD and immutable extent through its last native reader.
    /// A host supplies actual source/selection/Claim/aggregate-pin checks; this never mints authority.
    pub fn open(
        &self,
        source: Arc<dyn ArchiveSource>,
        offset: u64,
        length: u64,
    ) -> Result<Arc<dyn FileHandle>, Error> {
        self.control.check()?;
        source.check()?;
        let file = source.file()?;
        let metadata = file.metadata().map_err(|_| Error::Inventory)?;
        if !metadata.is_file()
            || offset
                .checked_add(length)
                .is_none_or(|end| end > metadata.len())
            || usize::try_from(length).is_err()
        {
            return Err(Error::Inventory);
        }
        let backing = Arc::new(Backing {
            source: source.clone(),
            version: Version::of(&metadata),
            ranges: BTreeMap::new(),
            control: self.control.clone(),
            read_limit: Some(self.window),
            reads: self.reads.clone(),
            _work: self.resources.clone(),
        });
        source.check()?;
        self.control.check()?;
        Ok(Arc::new(RangeHandle {
            backing,
            extent: knowledge_package::EntryRange { offset, length },
        }))
    }
}
