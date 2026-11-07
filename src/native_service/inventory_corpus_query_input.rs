//! One actual SDK Query, all selected-library funding and one pre-input DATA/deadline span.
//! No source FD/page is inspected until EVERY simultaneous input owner is already paid.
use super::super::super::super::InventoryCredit;
use super::*;

/// An unopened actual native library plus its independently owned, already-paid metadata loans.
/// The owner/selection/receiver are native capabilities; labels and text-root names grant nothing.
pub struct NativeQueryLibraryInput<'a> {
    service: &'a NativeLibraryService,
    owner: &'a Owner,
    text_root: &'a str,
    model: Option<NativeEmbeddingSelection>,
    reader: Option<Work>,
    inventory: Option<InventoryCredit>,
}
impl<'a> NativeQueryLibraryInput<'a> {
    pub fn new(
        service: &'a NativeLibraryService,
        owner: &'a Owner,
        text_root: &'a str,
        model: Option<NativeEmbeddingSelection>,
        reader: Work,
        inventory: Holding,
    ) -> Self {
        Self {
            service,
            owner,
            text_root,
            model,
            reader: Some(reader),
            inventory: Some(InventoryCredit(Some(inventory))),
        }
    }
}

/// Always retains the actual preparation owner, including every failure. Inputs are synchronous:
/// closed unused FDs/data precede their funding; no error code or partial descriptor is a grant.
pub struct NativeQueryInputs<'a> {
    failure: Option<anyhow::Error>,
    libraries: Vec<NativeQueryLibrary>,
    plans: Vec<NativeQueryLibraryInput<'a>>,
    permit: Permit,
    reads: ReadAllowance,
    deadline: Instant,
    scratch: InventoryCredit,
    funding: Option<Work>,
}
impl<'a> NativeQueryInputs<'a> {
    /// DATA quote for all tuple owners + actual selected-policy context costs, scratch4MiB
    /// and descriptor4MiB. Opens no input FD/page; a quote is NEVER resource admission.
    pub fn requirement(
        permit: &Permit,
        bindings: &[(&NativeLibraryService, &str)],
    ) -> Result<Reservation> {
        let wire::Command::Query { query } = permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        let mut selected = BTreeSet::new();
        let sdk_memory = permit.retained_input_memory().map_err(refused_error)?;
        let mut memory_bytes = (4 * 1024 * 1024u64)
            .checked_add(NativeTupleQuery::metadata_requirement().memory_bytes)
            .and_then(|n| n.checked_add(sdk_memory))
            .context("input quote overflow")?;
        for (service, text_root) in bindings {
            anyhow::ensure!(
                query.library_ids.contains(service.library_id())
                    && selected.insert(service.library_id().as_str()),
                "foreign/duplicate Query input quote"
            );
            let inventory = service.query_inventory_requirement(permit, text_root)?;
            memory_bytes = memory_bytes
                .checked_add(NativeTupleRead::requirement().memory_bytes)
                .and_then(|n| n.checked_add(inventory.memory_bytes))
                .context("complete Query input quote overflow")?;
        }
        anyhow::ensure!(
            selected.len() == query.library_ids.len(),
            "complete actual input quote required"
        );
        anyhow::ensure!(
            memory_bytes <= query.budget.max_memory_bytes.get(),
            "complete Query input peak exceeds actual request"
        );
        Ok(Reservation {
            memory_bytes,
            temporary_bytes: 0,
        })
    }
    pub fn open(
        permit: Permit,
        plans: Vec<NativeQueryLibraryInput<'a>>,
        scratch: Holding,
        funding: Work,
    ) -> Self {
        let started = Instant::now();
        let mut inputs = Self {
            failure: None,
            libraries: Vec::new(),
            plans,
            permit,
            reads: ReadAllowance::new(0),
            deadline: started,
            scratch: InventoryCredit(Some(scratch)),
            funding: Some(funding),
        };
        if let Err(error) = inputs.begin() {
            inputs.failure = Some(error);
        }
        inputs
    }
    fn check(&self) -> Result<()> {
        self.reads.check()?;
        self.permit.check().map_err(refused_error)?;
        anyhow::ensure!(Instant::now() < self.deadline, QueryBudgetExhausted);
        Ok(())
    }
    fn begin(&mut self) -> Result<()> {
        let wire::Command::Query { query } =
            self.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        self.deadline = self.permit.query_deadline().map_err(refused_error)?;
        let budget = Budget {
            max_files: query.budget.max_files.get(),
            max_bytes: query.budget.max_bytes.get(),
            max_millis: query.budget.max_millis.get(),
            max_memory_bytes: query.budget.max_memory_bytes.get(),
        };
        anyhow::ensure!(
            self.funding
                .as_ref()
                .context("actual descriptor owner")?
                .remaining()?
                .memory_bytes
                >= NativeTupleQuery::metadata_requirement().memory_bytes,
            "actual descriptor funding required"
        );
        anyhow::ensure!(
            self.scratch.remaining()?.memory_bytes >= 4 * 1024 * 1024,
            "complete input scratch is not prepaid"
        );
        let sdk_memory = self.permit.retained_input_memory().map_err(refused_error)?;
        let mut memory = self
            .funding
            .as_ref()
            .unwrap()
            .remaining()?
            .memory_bytes
            .checked_add(self.scratch.remaining()?.memory_bytes)
            .and_then(|n| n.checked_add(sdk_memory))
            .context("input memory overflow")?;
        let mut selected = BTreeSet::new();
        // Validate EVERY actual loan/selection before even the FIRST input factory runs.
        for plan in &self.plans {
            let library = plan.service.library_id();
            anyhow::ensure!(
                query.library_ids.contains(library) && selected.insert(library.as_str()),
                "foreign/duplicate Query input library"
            );
            let reader = plan
                .reader
                .as_ref()
                .context("actual tuple loan")?
                .remaining()?;
            let inventory = plan
                .inventory
                .as_ref()
                .context("actual inventory loan")?
                .remaining()?;
            anyhow::ensure!(
                reader.memory_bytes >= NativeTupleRead::requirement().memory_bytes
                    && inventory.memory_bytes >= 1024 * 1024
                    && reader.temporary_bytes == 0
                    && inventory.temporary_bytes == 0,
                "complete simultaneous Query input funding required"
            );
            memory = memory
                .checked_add(reader.memory_bytes)
                .and_then(|n| n.checked_add(inventory.memory_bytes))
                .context("input memory overflow")?;
        }
        anyhow::ensure!(
            selected.len() == query.library_ids.len(),
            "actual complete Query input library set required"
        );
        anyhow::ensure!(
            memory <= budget.max_memory_bytes,
            "actual simultaneous input peak exceeds request"
        );
        anyhow::ensure!(
            budget.max_files
                >= (self.plans.len() as u64)
                    .checked_mul(2)
                    .context("input object overflow")?,
            QueryBudgetExhausted
        );
        // Quote every ACTUAL selected policy before the FIRST input FD. A late library with
        // larger exclusions must not discover an unpaid context after earlier reads.
        for plan in &self.plans {
            let needed = plan
                .service
                .query_inventory_requirement(&self.permit, plan.text_root)?;
            anyhow::ensure!(
                plan.inventory.as_ref().unwrap().remaining()?.memory_bytes >= needed.memory_bytes,
                "actual selected-policy input peak is not fully prepaid"
            );
        }
        for plan in &self.plans {
            if query.methods.contains(&wire::SearchMethod::Semantic) {
                plan.model
                    .as_ref()
                    .context("actual selected semantic model owner")?
                    .check_permit(&self.permit)?;
            }
        }
        self.check()?;
        // Reverse/pop keeps the declared order without a second source-sized allocation.
        self.plans.reverse();
        while let Some(mut plan) = self.plans.pop() {
            self.check()?;
            let reader = self.reads.with_remaining(
                DataAmount {
                    bytes: budget.max_bytes,
                    objects: budget.max_files,
                },
                |left| {
                    let limits = Budget {
                        max_bytes: left.bytes,
                        max_files: left.objects,
                        max_millis: self
                            .deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis()
                            .min(u128::from(budget.max_millis))
                            as u64,
                        max_memory_bytes: budget.max_memory_bytes,
                    };
                    let reader = plan.service.open_corpus_tuple_limited(
                        plan.owner,
                        self.permit.clone(),
                        plan.reader.take().context("actual unopened tuple loan")?,
                        Some((limits, self.deadline)),
                    )?;
                    let spent = reader.physical.input_reads.amount();
                    Ok((reader, spent))
                },
            )?;
            self.check()?;
            let inventory = self.reads.with_remaining(
                DataAmount {
                    bytes: budget.max_bytes,
                    objects: budget.max_files,
                },
                |left| {
                    let limits = Budget {
                        max_bytes: left.bytes,
                        max_files: left.objects,
                        max_millis: self
                            .deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis()
                            .min(u128::from(budget.max_millis))
                            as u64,
                        max_memory_bytes: 4 * 1024 * 1024,
                    };
                    anyhow::ensure!(limits.max_millis > 0, QueryBudgetExhausted);
                    let inventory = plan.service.inventory_for_query(
                        plan.owner,
                        &self.permit,
                        plan.text_root,
                        &reader.physical.data.scan,
                        plan.inventory
                            .take()
                            .context("actual unopened inventory loan")?
                            .into_holding(),
                        &Preflight {
                            holding: &self.scratch,
                            budget: limits,
                        },
                    )?;
                    let spent = DataAmount {
                        bytes: inventory.owner.input_bytes,
                        objects: inventory.owner.input_objects,
                    };
                    Ok((inventory, spent))
                },
            )?;
            self.check()?;
            self.libraries.push(NativeQueryLibrary::new(
                reader,
                inventory,
                plan.model.take(),
            ));
        }
        Ok(())
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_first_input(&self) -> (u64, u64, Instant) {
        let first = &self.libraries[0];
        let tuple = first.reader.physical.input_reads.amount();
        (
            tuple.bytes + first.inventory.owner.input_bytes,
            tuple.objects + first.inventory.owner.input_objects,
            self.deadline,
        )
    }
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.failure.as_ref()
    }
    /// Exact DATA consumption, not memory credit, a completed-work receipt or permission.
    pub fn input_amount(&self) -> (u64, u64) {
        let used = self.reads.amount();
        (used.bytes, used.objects)
    }
    /// Consume ONLY the complete live preparation. Failed inputs remain an actual owning object;
    /// no byte-equal request, fresh per-library deadline or post-read budget check can replace it.
    pub fn finish(mut self) -> std::result::Result<NativeTupleQuery, Box<Self>> {
        if self.failure.is_some() {
            return Err(Box::new(self));
        }
        if let Err(error) = self.check() {
            self.failure = Some(error);
            return Err(Box::new(self));
        }
        let libraries = std::mem::take(&mut self.libraries);
        let funding = self.funding.take().expect("actual descriptor owner");
        match NativeTupleQuery::prepare(libraries, funding) {
            Ok(query) => {
                if query.cause.reads.amount() != self.reads.amount()
                    || query.cause.deadline != self.deadline
                {
                    self.failure = Some(anyhow::anyhow!(
                        "actual Query input accounting/deadline differs"
                    ));
                    return Err(Box::new(self));
                }
                Ok(query)
            }
            Err(error) => {
                self.failure = Some(error);
                Err(Box::new(self))
            }
        }
    }
}
