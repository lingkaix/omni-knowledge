//! Native selected-root and current-policy authority, separate from wire data.
//!
//! The trusted embedding owns `Owner` and supplies actual transport observations. Transport
//! adapters cannot mint selections or human approvals from decoded IDs, paths or token claims.
//! All sessions are agents. Human-only intent is a separate, one-use, exact-command capability.
use crate::{Command, ErrorCode, ExposureMode, Id, RelativePath, Transport};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, Mutex};

/// Unmodified observations from the actual transport, not a caller-supplied authentication DTO.
#[derive(Clone, Debug)]
pub enum TransportFacts {
    /// Actual OS peer and service effective UIDs, observed by the trusted IPC adapter.
    OwnerIpc {
        /// Native peer UID, never a request argument.
        peer_uid: u32,
        /// Native service UID, never a request argument.
        service_uid: u32,
    },
    /// Actual explicitly owner-launched direct stdio, not a daemon proxy connection.
    OwnerStdio,
    /// HTTP observations; loopback is a necessary listener condition, not identity.
    Http {
        /// Whether the actual listener/peer is loopback.
        loopback: bool,
        /// Unmodified Host field, including a port when present.
        host: String,
        /// Unmodified Origin field; None is not a wildcard browser grant.
        origin: Option<String>,
    },
}
impl TransportFacts {
    /// Return the actual face, without interpreting it as consent.
    fn transport(&self) -> Transport {
        match self {
            Self::OwnerIpc { .. } => Transport::OwnerIpc,
            Self::OwnerStdio => Transport::OwnerStdio,
            Self::Http { .. } => Transport::Http,
        }
    }
}

/// Explicit service-local policy installed by the native owner, never portable manifest approval.
#[derive(Clone, Debug)]
pub struct LocalPolicy {
    /// Admitted ordinary operation mode.
    pub mode: ExposureMode,
    /// Sensitive selections never use HTTP.
    pub sensitive: bool,
    /// Paths excluded before host enumeration, reads, processing and delivery.
    pub excluded_paths: Vec<RelativePath>,
    /// Native owner-approved faces; an empty list admits none.
    pub transports: Vec<Transport>,
    /// Literal HTTP Host allowlist; no suffix/URL normalization.
    pub http_hosts: Vec<String>,
    /// Literal HTTP Origin allowlist; missing Origin is permitted only for nonbrowser clients.
    pub http_origins: Vec<String>,
}

/// Owner-installed check of current native namespace/marker state, not a transport assertion.
pub type CurrentContext = Arc<dyn Fn(&File) -> bool + Send + Sync>;
/// Actual native source/container-version check retaining its real pins through delivery.
/// Must not capture/reenter this Permit/Owner or acquire its resource lock recursively.
pub type DeliveryCheck = Arc<dyn Fn() -> bool + Send + Sync>;
/// Current native selected-root state. A revision is a local sequence, never a digest ordering.
struct Selected {
    instance: Arc<()>,
    id: Id,
    root: File,
    identity: (u64, u64),
    revision: u64,
    policy: LocalPolicy,
    active: bool,
    current_context: Option<CurrentContext>,
}
/// One native selection. Its fields cannot be made from DTOs or reused across authority instances.
#[derive(Clone)]
pub struct Selection(Arc<Mutex<Selected>>);

/// Read-only native selection/current-configuration guard issued ONLY by that actual owner.
/// Not source/path, destination/write or transport authority; never construct it from DTOs.
#[derive(Clone)]
pub struct SelectionLease {
    selection: Selection,
    revision: u64,
}
impl SelectionLease {
    fn check_locked(&self, selected: &Selected) -> Result<(), ErrorCode> {
        if selected.revision != self.revision || !selected.active {
            return Err(ErrorCode::NotExposed);
        }
        let metadata = selected
            .root
            .metadata()
            .map_err(|_| ErrorCode::NotExposed)?;
        if !metadata.is_dir()
            || (metadata.dev(), metadata.ino()) != selected.identity
            || selected
                .current_context
                .as_ref()
                .is_some_and(|check| !check(&selected.root))
        {
            return Err(ErrorCode::NotExposed);
        }
        Ok(())
    }
    /// Check actual retained root/namespace/configuration epoch, not a label/hash/boolean.
    pub fn check(&self) -> Result<(), ErrorCode> {
        let selected = self.selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        self.check_locked(&selected)
    }
    /// Require the SAME opaque selection AND epoch in the live actual-command Permit.
    /// Equal library IDs in another authority/selection never substitute for this native owner.
    pub fn check_permit(&self, permit: &Permit) -> Result<(), ErrorCode> {
        permit.check()?;
        if !permit.revisions.iter().any(|(selection, revision)| {
            Arc::ptr_eq(&selection.0, &self.selection.0) && *revision == self.revision
        }) {
            return Err(ErrorCode::NotExposed);
        }
        self.check()
    }
}

/// Authority instance shared only with its separately retained native owner capability.
#[derive(Clone)]
pub struct NativeAuthority {
    instance: Arc<()>,
}
/// Startup/GUI owner capability. Never place this object in a transport or domain handler.
pub struct Owner {
    instance: Arc<()>,
}
impl Owner {
    /// Borrow the SAME native authority instance for another owner-selected library/runtime.
    /// This does not mint an owner/selection/transport grant or derive authority from data.
    pub fn authority(&self) -> NativeAuthority {
        NativeAuthority {
            instance: self.instance.clone(),
        }
    }
}
/// Native agent session restricted to exactly the owner-selected handles and actual face.
#[derive(Clone)]
pub struct Session {
    instance: Arc<()>,
    selections: Vec<Selection>,
    facts: TransportFacts,
    identity: Arc<()>,
}
/// One-use approval from the native owner intention flow, not a parsed `confirmed` flag.
pub struct HumanApproval {
    instance: Arc<()>,
    session: Arc<()>,
    command: Command,
    revisions: Vec<(Selection, u64)>,
}
/// Exact native intention shown to the owner, including the command's complete effect selection.
pub struct NativeIntent<'a> {
    /// Existing command catalog; no alternate confirmation vocabulary.
    pub command: &'a Command,
    /// Actual transport observations shown with the effect.
    pub transport: &'a TransportFacts,
}
/// Separately installed trusted native user interaction, never an implementation over request data.
pub trait HumanIntent {
    /// Present the complete intention and return only after actual owner confirmation.
    /// Refusal, stale UI, incomplete diff, or no available native UI must return an error.
    fn confirm(&mut self, intent: NativeIntent<'_>) -> std::io::Result<()>;
}

impl NativeAuthority {
    /// Establish an instance and its separate native owner; no selection or transport starts granted.
    pub fn new() -> (Self, Owner) {
        let instance = Arc::new(());
        (
            Self {
                instance: instance.clone(),
            },
            Owner { instance },
        )
    }
    /// Admit an agent session using native selections and unmodified observations.
    /// Selections must be from this instance's owner; all current policy checks run here and later.
    pub fn connect(
        &self,
        selections: &[Selection],
        facts: TransportFacts,
    ) -> Result<Session, ErrorCode> {
        let session = Session {
            instance: self.instance.clone(),
            selections: selections.to_vec(),
            facts,
            identity: Arc::new(()),
        };
        if selections.is_empty() {
            return Err(ErrorCode::NotExposed);
        }
        session.check(false)?;
        Ok(session)
    }
    /// Create a live permit only after scope resolution and exact native approval validation.
    pub(crate) fn admit(
        &self,
        session: &Session,
        command: &Command,
        libraries: &[Id],
        write: bool,
        owner_only: bool,
        approval: Option<HumanApproval>,
    ) -> Result<Permit, ErrorCode> {
        if !Arc::ptr_eq(&self.instance, &session.instance) {
            return Err(ErrorCode::NotExposed);
        }
        session.check(false)?;
        let revisions = session.revisions(libraries)?;
        let human = if let Some(approval) = approval {
            if !Arc::ptr_eq(&self.instance, &approval.instance)
                || !Arc::ptr_eq(&session.identity, &approval.session)
                || approval.command != *command
                || !same_revisions(&approval.revisions, &revisions)
            {
                return Err(ErrorCode::ConfirmationRequired);
            }
            true
        } else {
            false
        };
        if owner_only && !human {
            return Err(ErrorCode::OwnerRequired);
        }
        let permit = Permit {
            command: Arc::new(command.clone()),
            admitted_at: std::time::Instant::now(),
            session: session.clone(),
            revisions,
            write,
            human,
            resources: None,
            command_funding: None,
        };
        permit.check()?;
        Ok(permit)
    }
}

impl Owner {
    /// Pin the current native selection for a host-owned configuration/package guard.
    /// This gives no transport effect or arbitrary path authority. Caller prepays retained metadata.
    pub fn pin_selection(&self, selection: &Selection) -> Result<SelectionLease, ErrorCode> {
        selection.require_owner(&self.instance)?;
        let selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        let lease = SelectionLease {
            selection: selection.clone(),
            revision: selected.revision,
        };
        lease.check_locked(&selected)?;
        Ok(lease)
    }
    /// Commit one native owner's ALREADY VALIDATED configuration choice, compare-and-advancing
    /// the same selection epoch without copying/resetting current policy/exclusions. Failed model
    /// preparation leaves the previous epoch untouched. Old permits/guards cannot resurrect.
    pub fn revise_selection(&self, expected: &SelectionLease) -> Result<SelectionLease, ErrorCode> {
        expected.selection.require_owner(&self.instance)?;
        let mut selected = expected
            .selection
            .0
            .lock()
            .map_err(|_| ErrorCode::Internal)?;
        expected.check_locked(&selected)?;
        selected.revision = selected
            .revision
            .checked_add(1)
            .ok_or(ErrorCode::Internal)?;
        Ok(SelectionLease {
            selection: expected.selection.clone(),
            revision: selected.revision,
        })
    }
    /// Bind an actual selected directory descriptor and explicit current local policy.
    /// ID is a label only; the descriptor, instance and owner capability establish the selection.
    pub fn select(&self, id: Id, root: File, policy: LocalPolicy) -> Result<Selection, ErrorCode> {
        let metadata = root.metadata().map_err(|_| ErrorCode::Denied)?;
        if !metadata.is_dir() {
            return Err(ErrorCode::InvalidRequest);
        }
        // Keep this instance in the native selection; no decoded ID can substitute for it.
        let selected = Selection(Arc::new(Mutex::new(Selected {
            instance: self.instance.clone(),
            id,
            root,
            identity: (metadata.dev(), metadata.ino()),
            revision: 1,
            policy,
            active: true,
            current_context: None,
        })));
        Ok(selected)
    }
    /// Bind a native owner's current namespace/marker check, never a transport claim.
    /// All domain IO must still use the selected descriptor. The callback must not reenter authority.
    pub fn select_current(
        &self,
        id: Id,
        root: File,
        policy: LocalPolicy,
        current_context: CurrentContext,
    ) -> Result<Selection, ErrorCode> {
        if !current_context(&root) {
            return Err(ErrorCode::NotExposed);
        }
        let selection = self.select(id, root, policy)?;
        selection
            .0
            .lock()
            .map_err(|_| ErrorCode::Internal)?
            .current_context = Some(current_context);
        Ok(selection)
    }
    /// Replace current policy under the native owner capability, invalidating every old permit.
    pub fn set_policy(&self, selection: &Selection, policy: LocalPolicy) -> Result<(), ErrorCode> {
        selection.require_owner(&self.instance)?;
        let mut selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        selected.revision = selected
            .revision
            .checked_add(1)
            .ok_or(ErrorCode::Internal)?;
        selected.policy = policy;
        Ok(())
    }
    /// Revoke a native selection. Existing sessions/permits fail without clearing live work accounting.
    pub fn revoke(&self, selection: &Selection) -> Result<(), ErrorCode> {
        selection.require_owner(&self.instance)?;
        let mut selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        selected.revision = selected
            .revision
            .checked_add(1)
            .ok_or(ErrorCode::Internal)?;
        selected.active = false;
        Ok(())
    }
    /// Obtain one exact native confirmation; it is bound to the session, command and all policy revisions.
    /// The trusted caller must use the real owner UI and complete proposal/diff intention mapping.
    pub fn approve(
        &self,
        session: &Session,
        command: &Command,
        intention: &mut impl HumanIntent,
    ) -> Result<HumanApproval, ErrorCode> {
        if !Arc::ptr_eq(&self.instance, &session.instance) {
            return Err(ErrorCode::OwnerRequired);
        }
        if session.is_http() {
            return Err(ErrorCode::OwnerRequired);
        }
        session.check(false)?;
        let revisions = session.revisions(&[])?;
        intention
            .confirm(NativeIntent {
                command,
                transport: &session.facts,
            })
            .map_err(|_| ErrorCode::ConfirmationRequired)?;
        session.check(false)?;
        let current = session.revisions(&[])?;
        if !same_revisions(&revisions, &current) {
            return Err(ErrorCode::Changed);
        }
        Ok(HumanApproval {
            instance: self.instance.clone(),
            session: session.identity.clone(),
            command: command.clone(),
            revisions,
        })
    }
}

/// A retained authorization context; domain adapters must check it at every controllable effect/chunk.
#[derive(Clone)]
pub struct Permit {
    command: Arc<Command>,
    admitted_at: std::time::Instant,
    session: Session,
    revisions: Vec<(Selection, u64)>,
    write: bool,
    human: bool,
    resources: Option<Arc<crate::runtime::FrameResources>>,
    // Actual shared command bytes outlive the parent scope, without retaining its result pins.
    command_funding: Option<Arc<crate::runtime::FrameBudget>>,
}
/// Exact loan return either retains the still-borrowed scope, or reports a refusal AFTER
/// every scoped payload/pin was destroyed and its resource holding safely finished.
pub enum ScopeRejoinError {
    /// Another actual owner/pin still exists, or the requested cause is not current. No force-release.
    Retained {
        /// The actual scope and its still-paid pins; no ownership is discarded.
        scope: Box<Permit>,
        /// Why the synchronous return was refused.
        code: ErrorCode,
    },
    /// No scoped payload/pin remains. Rejoin was refused; the actual holding has been finished.
    Released(ErrorCode),
}
impl Permit {
    /// Borrow the EXACT command admitted by this native authority, after currentness checks.
    /// It is a retained observation, not a DTO-created grant; clones share the same command
    /// and actual frame holding rather than duplicating large request buffers/spending credit.
    pub fn admitted_command(&self) -> Result<&Command, ErrorCode> {
        self.check()?;
        Ok(&self.command)
    }
    /// The SAME original Query deadline across input preparation, waiting and scope forks.
    /// Timing DATA only: this cannot grant a source or prove resource/worker settlement.
    pub fn query_deadline(&self) -> Result<std::time::Instant, ErrorCode> {
        let Command::Query { query } = self.admitted_command()? else {
            return Err(ErrorCode::InvalidRequest);
        };
        let deadline = self
            .admitted_at
            .checked_add(std::time::Duration::from_millis(
                query.budget.max_millis.get(),
            ))
            .ok_or(ErrorCode::BudgetExhausted)?;
        if std::time::Instant::now() >= deadline {
            return Err(ErrorCode::BudgetExhausted);
        }
        Ok(deadline)
    }
    /// Actual shared frame identity for pre-input assembly, not equal request bytes or a grant.
    pub fn same_input_scope(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.command, &other.command)
            && match (&self.resources, &other.resources) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
    /// Actual retained SDK input memory; shared command/scope holding is counted ONCE.
    /// DATA observation only: cannot reserve, multiply, release or authorize any work.
    pub fn retained_input_memory(&self) -> Result<u64, ErrorCode> {
        self.check()?;
        let command = self
            .command_funding
            .as_ref()
            .ok_or(ErrorCode::BudgetExhausted)?;
        let scope = &self
            .resources
            .as_ref()
            .ok_or(ErrorCode::BudgetExhausted)?
            .budget;
        let memory = command.retained_memory()?;
        if Arc::ptr_eq(command, scope) {
            Ok(memory)
        } else {
            memory
                .checked_add(scope.retained_memory()?)
                .ok_or(ErrorCode::BudgetExhausted)
        }
    }
    /// Reserve retained aggregate/output bytes BEFORE cloning/growing buffers. The real holding
    /// follows every cloned permit and Delivery through the final transport write settlement.
    pub fn reserve_result(&self, bytes: u64) -> Result<(), ErrorCode> {
        self.check()?;
        self.resources
            .as_ref()
            .ok_or(ErrorCode::BudgetExhausted)?
            .grow(bytes)
    }
    /// Retain a real descriptor/container/generation pin and its native current-version check.
    /// This is a native host obligation, never an approval assertion accepted from a DTO.
    pub fn retain_check(&self, check: DeliveryCheck) -> Result<(), ErrorCode> {
        self.check()?;
        self.resources
            .as_ref()
            .ok_or(ErrorCode::BudgetExhausted)?
            .retain_check(check)
    }
    pub(crate) fn bind_resources(&mut self, resources: Arc<crate::runtime::FrameResources>) {
        if self.command_funding.is_none() {
            self.command_funding = Some(resources.budget.clone());
        }
        self.resources = Some(resources);
    }
    /// Fresh independent Source/Query scope over the EXACT already admitted command/session.
    /// This grants no new selection, command, raw-write or human intention. Old result/version pins
    /// are not copied into fresh IO; the host separately retains/checks its actual job/input cause.
    /// The complete scoped envelope is split from this actual live Claim BEFORE copies/FD opens.
    /// Its local spend counter can neither enlarge/refund credit nor fall back to global reserve.
    /// Drop nonwinning scopes promptly; move actual winning scopes/pins through final delivery.
    pub fn fork_processing_scope(
        &self,
        claim: &knowledge_core::jobs::Claim,
        expected: knowledge_core::jobs::Key<'_>,
        reservation: knowledge_core::jobs::Reservation,
    ) -> Result<Self, ErrorCode> {
        self.check()?;
        if !matches!(
            &*self.command,
            Command::Process { .. } | Command::Refresh { .. } | Command::Query { .. }
        ) {
            return Err(ErrorCode::InvalidRequest);
        }
        if claim
            .stop_requested(expected)
            .map_err(|_| ErrorCode::Changed)?
            .is_some()
        {
            return Err(ErrorCode::Changed);
        }
        let scope_count = self
            .revisions
            .len()
            .checked_add(self.session.selections.len())
            .ok_or(ErrorCode::BudgetExhausted)?;
        let copied_facts = match &self.session.facts {
            TransportFacts::Http { host, origin, .. } => host
                .len()
                .checked_add(origin.as_ref().map_or(0, String::len))
                .ok_or(ErrorCode::BudgetExhausted)?,
            _ => 0,
        };
        let need = (scope_count as u64)
            .checked_mul(64)
            .and_then(|n| n.checked_add((copied_facts as u64).checked_mul(8)?))
            .and_then(|n| n.checked_add(256 * 1024))
            .ok_or(ErrorCode::BudgetExhausted)?;
        if reservation.memory_bytes < need {
            return Err(ErrorCode::BudgetExhausted);
        }
        let command_funding = self
            .command_funding
            .as_ref()
            .ok_or(ErrorCode::BudgetExhausted)?
            .clone();
        let holding = knowledge_core::admission::Account::current()
            .map_err(|_| ErrorCode::BudgetExhausted)?
            .split_claim(claim, expected, reservation)
            .map_err(|_| ErrorCode::BudgetExhausted)?;
        let resources = Arc::new(crate::runtime::FrameResources::prepaid(holding, need)?);
        let fork = Self {
            command: self.command.clone(),
            admitted_at: self.admitted_at,
            session: self.session.clone(),
            revisions: self.revisions.clone(),
            write: self.write,
            human: self.human,
            resources: Some(resources),
            command_funding: Some(command_funding),
        };
        fork.check()?;
        Ok(fork)
    }
    /// Return ONLY this exclusively owned prepaid scope loan to the exact original live Claim.
    /// This permits bounded corpus iteration without releasing/re-reserving global credit between
    /// sources. Clones/deliveries/native pins must first actually die; shared command funding is
    /// separate. Cleanup does not require permission for another effect after policy revocation.
    pub fn rejoin_processing_scope(
        mut self,
        claim: &knowledge_core::jobs::Claim,
        expected: knowledge_core::jobs::Key<'_>,
    ) -> Result<(), ScopeRejoinError> {
        if claim.stop_requested(expected).is_err() {
            return Err(ScopeRejoinError::Retained {
                scope: Box::new(self),
                code: ErrorCode::Changed,
            });
        }
        let holding = self
            .resources
            .as_mut()
            .and_then(Arc::get_mut)
            .and_then(|r| r.take_prepaid());
        let Some(mut holding) = holding else {
            return Err(ScopeRejoinError::Retained {
                scope: Box::new(self),
                code: ErrorCode::Busy,
            });
        };
        // The real loan remains charged while ALL scope/session/revision/callback buffers die.
        drop(self);
        let result = knowledge_core::admission::Account::current()
            .and_then(|a| a.rejoin_claim(&mut holding, claim, expected));
        if result.is_err() {
            // There are now no live paid payloads or work to quarantine/abandon.
            let _ = holding.finish();
            return Err(ScopeRejoinError::Released(ErrorCode::Changed));
        }
        Ok(())
    }
    /// Recheck current native directory identity, face, sensitivity, mode and exact policy revision.
    pub fn check(&self) -> Result<(), ErrorCode> {
        self.session.check(self.write)?;
        let current = self.session.revisions(&[])?;
        if !same_revisions(&self.revisions, &current) {
            return Err(ErrorCode::NotExposed);
        }
        if let Some(resources) = &self.resources {
            resources.check()?;
        }
        Ok(())
    }
    /// Project current selected exposure without disclosing excluded paths or local HTTP allowlists.
    pub fn visible_exposure(&self, library: &Id) -> Result<crate::Exposure, ErrorCode> {
        self.check()?;
        let selected = self
            .session
            .find(library)?
            .0
            .lock()
            .map_err(|_| ErrorCode::Internal)?;
        Ok(crate::Exposure {
            mode: selected.policy.mode,
            sensitive: selected.policy.sensitive,
            excluded_paths: vec![],
            transports: vec![self.session.facts.transport()],
            revision: Id::new(format!("native-policy-{}", selected.revision))
                .map_err(|_| ErrorCode::Internal)?,
            pending_expansion: false,
        })
    }
    /// Whether a selected root needs an exclusion-aware statistical generation before lexical query.
    /// This is a native host fact, never a portable permission boolean.
    pub fn has_exclusions(&self, library: &Id) -> Result<bool, ErrorCode> {
        self.check()?;
        Ok(!self
            .session
            .find(library)?
            .0
            .lock()
            .map_err(|_| ErrorCode::Internal)?
            .policy
            .excluded_paths
            .is_empty())
    }
    /// Actual network face, for removing native root/cache paths from public projections.
    pub fn is_http(&self) -> bool {
        self.session.is_http()
    }
    /// Inspect actual local exposure only for an exact native human-approved invocation.
    pub fn exposure(&self, library: &Id) -> Result<crate::Exposure, ErrorCode> {
        self.check()?;
        if !self.human {
            return Err(ErrorCode::OwnerRequired);
        }
        let selected = self
            .session
            .find(library)?
            .0
            .lock()
            .map_err(|_| ErrorCode::Internal)?;
        Ok(crate::Exposure {
            mode: selected.policy.mode,
            sensitive: selected.policy.sensitive,
            excluded_paths: selected.policy.excluded_paths.clone(),
            transports: selected.policy.transports.clone(),
            revision: Id::new(format!("native-policy-{}", selected.revision))
                .map_err(|_| ErrorCode::Internal)?,
            pending_expansion: false,
        })
    }
    /// Whether this exact invocation carries native human confirmation; agents otherwise propose.
    pub fn is_human(&self) -> bool {
        self.human
    }
    /// Check that a domain-resolved library is one of the actual native selections.
    pub fn check_library(&self, library: &Id) -> Result<(), ErrorCode> {
        self.check()?;
        self.session.find(library)?;
        Ok(())
    }
    /// Check a native-resolved source/target path against current exclusions before reading/effects.
    pub fn check_path(&self, library: &Id, path: &RelativePath) -> Result<(), ErrorCode> {
        self.check()?;
        let selection = self.session.find(library)?;
        let selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        if selected.policy.excluded_paths.iter().any(|p| {
            path.as_str() == p.as_str()
                || path
                    .as_str()
                    .strip_prefix(p.as_str())
                    .is_some_and(|s| s.starts_with('/'))
        }) {
            return Err(ErrorCode::NotExposed);
        }
        Ok(())
    }
    /// Execute against the held selected directory with policy revision locked across the effect.
    /// The closure must enforce storage current-version/child-root rules, use descriptor-confined IO,
    /// and must not reenter this permit/owner. This does not authorize arbitrary paths under the root.
    pub fn with_root<T>(
        &self,
        library: &Id,
        effect: impl FnOnce(&File) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.with_selected_context(library, |root, _, _| effect(root))
    }
    /// Borrow actual current selected FD, read policy and revision under the same native lock.
    /// No path is thereby authorized; check_path is still required. Do not reenter this owner/permit.
    pub fn with_selected_context<T>(
        &self,
        library: &Id,
        effect: impl FnOnce(&File, &LocalPolicy, u64) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.check()?;
        let selection = self.session.find(library)?;
        let selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
        let expected = self
            .revisions
            .iter()
            .find(|(s, _)| Arc::ptr_eq(&s.0, &selection.0))
            .ok_or(ErrorCode::NotExposed)?
            .1;
        if selected.revision != expected || !selected.active {
            return Err(ErrorCode::NotExposed);
        }
        if selected
            .current_context
            .as_ref()
            .is_some_and(|check| !check(&selected.root))
        {
            return Err(ErrorCode::NotExposed);
        }
        let result = effect(&selected.root, &selected.policy, selected.revision)?;
        if selected
            .current_context
            .as_ref()
            .is_some_and(|check| !check(&selected.root))
        {
            return Err(ErrorCode::NotExposed);
        }
        Ok(result)
    }
}

impl Selection {
    /// Verify that only this selection's native owner/session instance can use it.
    fn require_owner(&self, instance: &Arc<()>) -> Result<(), ErrorCode> {
        let selected = self.0.lock().map_err(|_| ErrorCode::Internal)?;
        if Arc::ptr_eq(&selected.instance, instance) {
            Ok(())
        } else {
            Err(ErrorCode::NotExposed)
        }
    }
}

impl Session {
    /// Whether the actual face is HTTP; this is not a selected-root or owner grant.
    pub(crate) fn is_http(&self) -> bool {
        matches!(self.facts, TransportFacts::Http { .. })
    }
    /// Validate every selected native root and face without trusting an ID or cached revision.
    fn check(&self, write: bool) -> Result<(), ErrorCode> {
        for selection in &self.selections {
            selection.require_owner(&self.instance)?;
            let selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
            let metadata = selected
                .root
                .metadata()
                .map_err(|_| ErrorCode::NotExposed)?;
            if !selected.active
                || metadata.nlink() == 0
                || (metadata.dev(), metadata.ino()) != selected.identity
                || selected
                    .current_context
                    .as_ref()
                    .is_some_and(|check| !check(&selected.root))
            {
                return Err(ErrorCode::NotExposed);
            }
            let policy = &selected.policy;
            if policy.mode == ExposureMode::Off
                || !policy.transports.contains(&self.facts.transport())
            {
                return Err(ErrorCode::NotExposed);
            }
            if write && policy.mode != ExposureMode::ReadWrite {
                return Err(ErrorCode::ReadOnly);
            }
            match &self.facts {
                TransportFacts::OwnerIpc {
                    peer_uid,
                    service_uid,
                } if peer_uid != service_uid => return Err(ErrorCode::NotExposed),
                TransportFacts::Http {
                    loopback,
                    host,
                    origin,
                } if policy.sensitive
                    || !loopback
                    || !policy.http_hosts.contains(host)
                    || origin
                        .as_ref()
                        .is_some_and(|o| !policy.http_origins.contains(o)) =>
                {
                    return Err(ErrorCode::NotExposed);
                }
                _ => {}
            }
        }
        Ok(())
    }
    /// Find only native selected roots, using the DTO ID solely as a lookup label.
    fn find(&self, id: &Id) -> Result<&Selection, ErrorCode> {
        self.selections
            .iter()
            .find(|s| s.0.lock().is_ok_and(|s| s.id == *id))
            .ok_or(ErrorCode::NotExposed)
    }
    /// Capture all selected revisions; named scopes must be a subset and duplicate IDs refuse.
    fn revisions(&self, libraries: &[Id]) -> Result<Vec<(Selection, u64)>, ErrorCode> {
        for id in libraries {
            self.find(id)?;
        }
        let mut ids = Vec::new();
        let mut result = Vec::new();
        for selection in &self.selections {
            let selected = selection.0.lock().map_err(|_| ErrorCode::Internal)?;
            if ids.contains(&selected.id) {
                return Err(ErrorCode::NotExposed);
            }
            ids.push(selected.id.clone());
            result.push((selection.clone(), selected.revision));
        }
        Ok(result)
    }
}

/// Compare native capability identities and exact revisions, never revision ordering.
fn same_revisions(a: &[(Selection, u64)], b: &[(Selection, u64)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|((a, av), (b, bv))| Arc::ptr_eq(&a.0, &b.0) && av == bv)
}
