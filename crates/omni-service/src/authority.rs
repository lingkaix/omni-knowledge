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

/// Authority instance shared only with its separately retained native owner capability.
#[derive(Clone)]
pub struct NativeAuthority {
    instance: Arc<()>,
}
/// Startup/GUI owner capability. Never place this object in a transport or domain handler.
pub struct Owner {
    instance: Arc<()>,
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
            session: session.clone(),
            revisions,
            write,
            human,
            resources: None,
        };
        permit.check()?;
        Ok(permit)
    }
}

impl Owner {
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
    session: Session,
    revisions: Vec<(Selection, u64)>,
    write: bool,
    human: bool,
    resources: Option<Arc<crate::runtime::FrameResources>>,
}
impl Permit {
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
        self.resources = Some(resources);
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
