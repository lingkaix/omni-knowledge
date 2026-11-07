//! Native M1 inventory consumption. Journal DTOs cannot construct these opaque source carriers.
#[path = "inventory_job.rs"]
mod job;
#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
use super::source::refused_error;
use super::*;
pub(super) use job::RefreshCause;
pub use job::{
    NativeBm25Run, NativeCorpusCoverage, NativeCorpusDraft, NativeCorpusError,
    NativeCorpusProgress, NativePartitionResult, NativeQueryEmbedding, NativeQueryLibrary,
    NativeQueryLoans, NativeQueryPageLoan, NativeQueryPageRefusal, NativeQueryPartition,
    NativeQueryPartitionPage, NativeQuerySource, NativeQueryStage, NativeQueryStageRefusal,
    NativeQueryStageResources, NativeQueryStep, NativeRefreshJob, NativeRefreshScopeError,
    NativeSemanticRun, NativeTupleQuery, NativeTupleRead,
};
use knowledge_core::{
    admission::Holding,
    backend::{Availability, Budget},
};
use knowledge_local::{
    CurrentAdmission, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision,
};
use knowledge_storage::{
    ChangeKind, Observation, Preflight, ReconcileOptions, Reconciled, Replay, Schedule, Store,
    WriteContext, WritePolicy,
};
use omni_service::authority::SelectionLease;
use std::sync::Arc;
#[cfg(test)]
thread_local! {static TEST_QUERY_INPUT_OBJECTS:std::cell::Cell<u64>=const{std::cell::Cell::new(0)};}

/// Actual native inventory owner, immutable context epoch and complete persisted scan.
/// This is neither a serialized observation nor permission to select another library/receiver.
pub struct NativeInventory {
    owner: Arc<InventoryOwner>,
}
/// Drop only after the owner's actual metadata/FD fields. Holding itself deliberately does not
/// auto-release: this small guard finishes known-settled synchronous inventory ownership only.
struct InventoryCredit(Option<Holding>);
impl std::ops::Deref for InventoryCredit {
    type Target = Holding;
    fn deref(&self) -> &Holding {
        self.0.as_ref().expect("owned inventory credit")
    }
}
impl InventoryCredit {
    fn into_holding(mut self) -> Holding {
        self.0.take().expect("owned inventory credit")
    }
}
impl Drop for InventoryCredit {
    fn drop(&mut self) {
        if let Some(holding) = &mut self.0 {
            let _ = holding.finish();
        }
    }
}
struct InventoryOwner {
    store: Store<InventoryPolicy>,
    mount: LocalMount<InventoryPolicy>,
    policy: InventoryPolicy,
    scan: String,
    root: (u64, u64),
    check_budget: Budget,
    input_bytes: u64,
    input_objects: u64,
    // Current opaque owner epoch and actual context credit survive every dependent page/source.
    // Never retain a Permit here: a source's delivery checks may retain this inventory.
    _funding: InventoryCredit,
}
#[derive(Clone)]
struct InventoryPolicy(Arc<PolicyOwner>);
struct PolicyOwner {
    lease: SelectionLease,
    write: WriteContext,
    read: CurrentAdmission,
}
impl WritePolicy for InventoryPolicy {
    fn current(&self) -> std::result::Result<WriteContext, knowledge_storage::Error> {
        self.0
            .lease
            .check()
            .map_err(|_| knowledge_storage::Error::Context)?;
        Ok(self.0.write.clone())
    }
}
impl LocalReadPolicy for InventoryPolicy {
    fn decide(&self) -> ReadDecision {
        if self.0.lease.check().is_err() {
            ReadDecision::Deny
        } else {
            ReadDecision::Admit(self.0.read.clone())
        }
    }
}
/// One bounded replay page. Its actual metadata/credit survive all extracted opaque row handles.
pub struct NativeInventoryPage {
    page: Arc<PageOwner>,
}
struct PageOwner {
    replay: Replay,
    inventory: Arc<InventoryOwner>,
    _funding: InventoryCredit,
}
/// An actual row read from the selected M1 journal, not a caller-manufactured ID/stat DTO.
/// Its stable asset identity is still DATA; opening requires a separate current native read permit.
pub struct NativeObservedSource {
    page: Arc<PageOwner>,
    row: usize,
}
impl NativeInventoryPage {
    /// Validated next byte position; partial pages must follow this actual cursor without skipping.
    pub fn next_offset(&self) -> u64 {
        self.page.replay.next_offset
    }
    /// Native read spend including predecessor/read-ahead, not a logical cursor delta.
    pub fn read_bytes(&self) -> u64 {
        self.page.replay.read_bytes
    }
    /// Existing M1 metadata-object units, distinct from returned files/representations.
    pub fn read_objects(&self) -> u64 {
        (self.page.replay.observations.len()
            + self.page.replay.checkpoints.len()
            + self.page.replay.event_cursors.len()) as u64
    }
    /// True only when this actual journal reader reached its EOF.
    pub fn complete(&self) -> bool {
        self.page.replay.complete
    }
    /// Retained malformed/contradictory evidence is not successful empty coverage.
    pub fn issue(&self) -> Option<&knowledge_storage::Error> {
        self.page.replay.issue.as_ref()
    }
    /// Consume the page once; old scans/tombstones never become current producer inputs.
    /// Metadata remains shared/paid rather than copied into a resident corpus table.
    pub fn into_sources(self) -> impl Iterator<Item = NativeObservedSource> {
        Self::source_rows(self.page)
    }
    /// Stream borrowed-page handles while retaining the page object for affine funding recovery.
    /// Returned row handles still own the SAME actual paid metadata, not copies or DTO grants.
    pub fn sources(&self) -> impl Iterator<Item = NativeObservedSource> {
        Self::source_rows(self.page.clone())
    }
    /// Recover this ONE actual holding without global release/reserve, only after every row/source
    /// dependent is gone. Refusal returns the owning page unchanged. No credit refund/pool exists.
    pub fn into_funding(self) -> std::result::Result<Holding, Self> {
        let owner = Arc::try_unwrap(self.page).map_err(|page| Self { page })?;
        let PageOwner {
            replay,
            inventory,
            _funding,
        } = owner;
        drop(replay);
        drop(inventory);
        Ok(_funding.into_holding())
    }
    fn source_rows(page: Arc<PageOwner>) -> impl Iterator<Item = NativeObservedSource> {
        (0..page.replay.observations.len()).filter_map(move |row| {
            let observation = &page.replay.observations[row];
            (observation.scan == page.inventory.scan
                && observation.root == page.inventory.root
                && observation.change != ChangeKind::Deleted
                && observation.availability == Availability::Available)
                .then(|| NativeObservedSource {
                    page: page.clone(),
                    row,
                })
        })
    }
}
impl NativeObservedSource {
    /// Stable independently observed M1 asset data identity, not a path/content hash.
    pub fn source_id(&self) -> &str {
        &self.observation().source_id
    }
    /// Actual metadata observation; borrowing it creates no source read/write permission.
    pub fn observation(&self) -> &Observation {
        &self.page.replay.observations[self.row]
    }
    pub(super) fn check_file(&self, file: &File) -> Result<()> {
        let inventory = &self.page.inventory;
        inventory.policy.0.lease.check().map_err(refused_error)?;
        inventory.mount.check_held_snapshot(
            file,
            &self.observation().stat,
            inventory.check_budget,
        )?;
        Ok(())
    }
    pub(super) fn check_permit(&self, permit: &Permit) -> Result<()> {
        self.page
            .inventory
            .policy
            .0
            .lease
            .check_permit(permit)
            .map_err(refused_error)?;
        Ok(())
    }
}
impl NativeInventory {
    #[cfg(test)]
    pub(super) fn test_input_objects() -> u64 {
        TEST_QUERY_INPUT_OBJECTS.get()
    }
    /// Replay one paid page from the actual durable M1 stream; never accumulate journal history.
    /// The incoming holding covers the complete page plus wrapper before native inspection.
    pub fn page(
        &self,
        offset: u64,
        budget: Budget,
        funding: Holding,
    ) -> Result<NativeInventoryPage> {
        let funding = InventoryCredit(Some(funding));
        let replay = self.replay_page(offset, budget, &funding)?;
        Ok(self.paid_page(replay, funding))
    }
    // SAME decoder/FD/permissions, but an aggregate caller gets its actual incoming loan back.
    pub(super) fn page_owning(
        &self,
        offset: u64,
        budget: Budget,
        funding: Holding,
    ) -> std::result::Result<NativeInventoryPage, (anyhow::Error, Holding)> {
        match self.replay_page(offset, budget, &funding) {
            Ok(replay) => Ok(self.paid_page(replay, InventoryCredit(Some(funding)))),
            Err(error) => Err((error, funding)),
        }
    }
    fn paid_page(&self, replay: Replay, funding: InventoryCredit) -> NativeInventoryPage {
        NativeInventoryPage {
            page: Arc::new(PageOwner {
                replay,
                inventory: self.owner.clone(),
                _funding: funding,
            }),
        }
    }
    fn replay_page(&self, offset: u64, budget: Budget, funding: &Holding) -> Result<Replay> {
        anyhow::ensure!(
            funding.remaining()?.memory_bytes
                >= budget
                    .max_memory_bytes
                    .checked_add(128 * 1024)
                    .context("inventory page envelope overflow")?,
            "native inventory page is not fully prepaid"
        );
        self.owner.policy.0.lease.check().map_err(refused_error)?;
        let replay = self.owner.store.replay_tracking_readonly(
            &self.owner.mount,
            offset,
            &Preflight {
                holding: funding,
                budget,
            },
        )?;
        self.owner.policy.0.lease.check().map_err(refused_error)?;
        Ok(replay)
    }
    /// Actual complete durable scan ID for progress, never authorization or a global writer clock.
    pub fn scan(&self) -> &str {
        &self.owner.scan
    }
}
impl NativeLibraryService {
    // One root/policy/copy-quote path for actual Refresh and actual readonly Query intent.
    fn inventory_bindings(
        &self,
        owner: &Owner,
        permit: &Permit,
        text_root: &str,
        context_funding: &InventoryCredit,
        writing: bool,
    ) -> Result<(
        Store<InventoryPolicy>,
        LocalMount<InventoryPolicy>,
        InventoryPolicy,
    )> {
        anyhow::ensure!(
            matches!(text_root, "pages" | "wiki"),
            "invalid native text layout"
        );
        let intent = permit.admitted_command().map_err(refused_error)?;
        anyhow::ensure!(
            if writing {
                matches!(intent,wire::Command::Refresh{library_id,..} if library_id==&self.library)
            } else {
                matches!(intent,wire::Command::Query{query} if query.library_ids.contains(&self.library))
            },
            "actual matching inventory intent required"
        );
        let lease = owner
            .pin_selection(&self.selection)
            .map_err(refused_error)?;
        lease.check_permit(permit).map_err(refused_error)?;
        let host = self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("native library unavailable"))?;
        anyhow::ensure!(
            host.kb.config.library_id.is_some(),
            "native M1 inventory requires explicit durable library identity upgrade"
        );
        let root_path = &host.kb.root;
        let (store_root, mount_root, policy) = permit
            .with_selected_context(&self.library, |root, actual, revision| {
                let variable = actual
                    .excluded_paths
                    .iter()
                    .try_fold(0u64, |n, p| {
                        n.checked_add(128)?
                            .checked_add((p.as_str().len() as u64).checked_mul(16)?)
                    })
                    .and_then(|n| {
                        n.checked_add(
                            (root_path.as_os_str().len() as u64
                                + text_root.len() as u64
                                + self.library.as_str().len() as u64)
                                * 16,
                        )
                    });
                let needed = variable
                    .and_then(|n| n.checked_add(1024 * 1024))
                    .ok_or(wire::ErrorCode::BudgetExhausted)?;
                if context_funding.remaining().map_err(refused)?.memory_bytes < needed
                    || (if writing {
                        actual.mode != wire::ExposureMode::ReadWrite
                    } else {
                        actual.mode == wire::ExposureMode::Off
                    })
                {
                    return Err(wire::ErrorCode::BudgetExhausted);
                }
                let write = WriteContext {
                    library_id: self.library.as_str().into(),
                    revision,
                    text_root: text_root.into(),
                    writable: writing,
                    raw_original_writes: false,
                    outside_scope: actual
                        .excluded_paths
                        .iter()
                        .map(|p| p.as_str().to_owned())
                        .collect(),
                };
                let mut outside_scope = actual
                    .excluded_paths
                    .iter()
                    .map(|p| knowledge_core::data::RelativePath::new(p.as_str()).map_err(refused))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                // Canonical native bookkeeping is never original content or a source identity.
                for path in [".omni", ".git", "journal"] {
                    outside_scope
                        .push(knowledge_core::data::RelativePath::new(path).map_err(refused)?);
                }
                let read = CurrentAdmission {
                    library_id: self.library.as_str().into(),
                    mount_id: None,
                    revision,
                    locality: LocalityPremise::OrdinaryLocal,
                    outside_scope,
                };
                Ok((
                    root.try_clone().map_err(refused)?,
                    root.try_clone().map_err(refused)?,
                    InventoryPolicy(Arc::new(PolicyOwner {
                        lease: lease.clone(),
                        write,
                        read,
                    })),
                ))
            })
            .map_err(refused_error)?;
        let store = Store::from_selected_directory(
            root_path,
            store_root,
            self.library.as_str(),
            "native",
            policy.clone(),
        )?;
        let mount = LocalMount::from_selected_directory(
            root_path,
            mount_root,
            self.library.as_str(),
            None,
            policy.clone(),
        )?;
        drop(host);
        Ok((store, mount, policy))
    }
    /// Bind an existing complete M1 snapshot under actual Query read intent, without filesystem
    /// reconciliation, journal creation/upgrade, or a manufactured writable policy. Scan labels grant nothing.
    /// Context/replay scratch are prepaid; existing checkpoint evidence and total read bounds are checked.
    /// The caller retains the borrowed actual Query owner through subsequent paging and delivery.
    pub fn inventory_for_query(
        &self,
        owner: &Owner,
        permit: &Permit,
        text_root: &str,
        scan: &str,
        context_funding: Holding,
        preflight: &Preflight<'_>,
    ) -> Result<NativeInventory> {
        let context_funding = InventoryCredit(Some(context_funding));
        anyhow::ensure!(
            context_funding.remaining()?.memory_bytes >= 1024 * 1024,
            "native query inventory context is not prepaid"
        );
        anyhow::ensure!(
            scan.len() <= 128 && uuid::Uuid::parse_str(scan).is_ok(),
            "invalid snapshot scan identity data"
        );
        let wire::Command::Query { query } = permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        let requested = preflight.budget;
        anyhow::ensure!(
            requested.max_files <= query.budget.max_files.get()
                && requested.max_bytes <= query.budget.max_bytes.get()
                && requested.max_millis <= query.budget.max_millis.get()
                && requested.max_memory_bytes <= query.budget.max_memory_bytes.get(),
            "query inventory read exceeds actual Query budget"
        );
        let (store, mount, policy) =
            self.inventory_bindings(owner, permit, text_root, &context_funding, false)?;
        let root = mount.root_identity()?;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(requested.max_millis))
            .context("read inventory deadline")?;
        let mut offset = 0u64;
        let mut bytes = 0u64;
        let mut records = 0u64;
        let mut found = false;
        loop {
            permit.check().map_err(refused_error)?;
            let remaining = Budget {
                max_files: requested.max_files.saturating_sub(records),
                max_bytes: requested.max_bytes.saturating_sub(bytes),
                max_millis: deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u128::from(requested.max_millis)) as u64,
                max_memory_bytes: requested.max_memory_bytes,
            };
            anyhow::ensure!(
                remaining.max_files > 0 && remaining.max_bytes > 0 && remaining.max_millis > 0,
                knowledge_storage::Error::Budget
            );
            let replay = store.replay_tracking_readonly(
                &mount,
                offset,
                &Preflight {
                    holding: preflight.holding,
                    budget: remaining,
                },
            )?;
            #[cfg(test)]
            TEST_QUERY_INPUT_OBJECTS.set(
                TEST_QUERY_INPUT_OBJECTS.get()
                    + (replay.observations.len()
                        + replay.checkpoints.len()
                        + replay.event_cursors.len()) as u64,
            );
            anyhow::ensure!(
                Instant::now() < deadline && replay.issue != Some(knowledge_storage::Error::Budget),
                knowledge_storage::Error::Budget
            );
            anyhow::ensure!(
                replay.issue.is_none(),
                "query inventory evidence requires recovery"
            );
            bytes = bytes
                .checked_add(replay.read_bytes)
                .context("query inventory bytes overflow")?;
            records = records
                .checked_add(
                    (replay.observations.len()
                        + replay.checkpoints.len()
                        + replay.event_cursors.len()) as u64,
                )
                .context("query inventory records overflow")?;
            anyhow::ensure!(
                bytes <= requested.max_bytes && records <= requested.max_files,
                knowledge_storage::Error::Budget
            );
            for checkpoint in &replay.checkpoints {
                if checkpoint.scan == scan {
                    anyhow::ensure!(
                        checkpoint.complete
                            && checkpoint.root == root
                            && checkpoint.backend == mount.context(),
                        "incomplete/foreign snapshot checkpoint"
                    );
                    found = true;
                }
            }
            if replay.complete {
                break;
            }
            anyhow::ensure!(
                replay.next_offset > offset,
                "query inventory replay did not advance"
            );
            offset = replay.next_offset;
        }
        anyhow::ensure!(
            found,
            "selected query snapshot has no actual complete M1 checkpoint"
        );
        permit.check().map_err(refused_error)?;
        Ok(NativeInventory {
            owner: Arc::new(InventoryOwner {
                store,
                mount,
                policy,
                scan: scan.to_owned(),
                root,
                input_bytes: bytes,
                input_objects: records,
                check_budget: Budget {
                    max_files: 1,
                    max_bytes: 128 * 1024,
                    max_millis: requested.max_millis,
                    max_memory_bytes: 128 * 1024,
                },
                _funding: context_funding,
            }),
        })
    }
    /// Consume the actual Refresh intention through M1 before full-corpus processing.
    /// Native Owner selects the text layout; no source/read grant implies raw-original writes.
    /// Context and scan scratch are BOTH already prepaid in the ONE account before inspection.
    /// Old inventory/source handles are revoked by selection/policy/model epoch changes.
    pub fn reconcile_inventory(
        &self,
        owner: &Owner,
        permit: Permit,
        text_root: &str,
        context_funding: Holding,
        preflight: &Preflight<'_>,
    ) -> Result<(NativeInventory, Reconciled)> {
        let context_funding = InventoryCredit(Some(context_funding));
        anyhow::ensure!(
            context_funding.remaining()?.memory_bytes >= 1024 * 1024,
            "native inventory context is not prepaid"
        );
        let wire::Command::Refresh {
            library_id,
            verification,
            budget,
            ..
        } = permit.admitted_command().map_err(refused_error)?
        else {
            bail!("M1 inventory requires the actual Refresh command");
        };
        anyhow::ensure!(
            library_id == &self.library,
            "inventory library differs from command"
        );
        let requested = preflight.budget;
        anyhow::ensure!(
            requested.max_files <= budget.max_files.get()
                && requested.max_bytes <= budget.max_bytes.get()
                && requested.max_millis <= budget.max_millis.get()
                && requested.max_memory_bytes <= budget.max_memory_bytes.get(),
            "inventory scan exceeds the actual Refresh budget"
        );
        anyhow::ensure!(
            matches!(text_root, "pages" | "wiki"),
            "invalid native text layout"
        );
        let verify_bytes = *verification == wire::Verification::FullHash;
        let (store, mount, policy) =
            self.inventory_bindings(owner, &permit, text_root, &context_funding, true)?;
        let root = mount.root_identity()?;
        let reconciled = store.reconcile_local(
            &mount,
            ReconcileOptions {
                verify_bytes,
                schedule: Schedule::Manual,
            },
            preflight,
        )?;
        permit.check().map_err(refused_error)?;
        anyhow::ensure!(
            reconciled.complete,
            "native inventory scan is partial; previous generation remains selected"
        );
        let scan = reconciled
            .last_complete
            .clone()
            .context("native inventory lacks durable checkpoint")?;
        let check_budget = Budget {
            max_files: 1,
            max_bytes: 128 * 1024,
            max_millis: requested.max_millis,
            max_memory_bytes: 128 * 1024,
        };
        Ok((
            NativeInventory {
                owner: Arc::new(InventoryOwner {
                    store,
                    mount,
                    policy,
                    scan,
                    root,
                    input_bytes: 0,
                    input_objects: 0,
                    check_budget,
                    _funding: context_funding,
                }),
            },
            reconciled,
        ))
    }
    /// Open this actual M1 row through a SEPARATE current read/Process/Refresh permit.
    /// Actual retained descriptor identity must match M1 native metadata even for identical bytes.
    pub fn source_for_observed(
        &self,
        permit: &Permit,
        observed: NativeObservedSource,
        verification: Budget,
    ) -> Result<NativeSource> {
        observed.check_permit(permit)?;
        anyhow::ensure!(
            observed.observation().stat.backend.library_id == self.library.as_str(),
            "observed source belongs to another library"
        );
        let allowed = match permit.admitted_command().map_err(refused_error)? {
            wire::Command::Process {
                library_id,
                source_ids,
                ..
            } => {
                library_id == &self.library
                    && source_ids
                        .iter()
                        .any(|id| id.as_str() == observed.source_id())
            }
            wire::Command::Refresh { library_id, .. } => library_id == &self.library,
            _ => false,
        };
        anyhow::ensure!(
            allowed,
            "actual command does not select this M1 processing source"
        );
        let path = wire::RelativePath::new(observed.observation().stat.path.as_str())?;
        NativeSource::open(&self.library, &path, permit, verification)?.bind_observed(observed)
    }
}
