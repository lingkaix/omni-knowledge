//! Builder tests against actual native selections and filesystem observations, not transport mocks.
use super::*;
use crate::authority::{HumanIntent, LocalPolicy, NativeIntent, Owner, Selection, TransportFacts};
use crate::{EntryKind, ExposureMode, ListingEntry, Transport};
use knowledge_core::admission::{Configuration, Scheduling};
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Private process startup fixture. The parent runner removes this exact directory after process exit.
fn account() -> &'static Account {
    static STARTED: OnceLock<&'static Account> = OnceLock::new();
    STARTED.get_or_init(|| {
        let directory = tempfile::Builder::new()
            .prefix("native-authority-account-")
            .tempdir()
            .unwrap()
            .keep();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        Account::start(
            File::open(directory).unwrap(),
            Configuration {
                memory_bytes: Some(2 * 1024 * 1024 * 1024),
                storage_bytes: None,
            },
            Scheduling {
                max_pending: 64,
                max_records: 256,
                max_descriptor_bytes: 1024 * 1024,
                per_mount: 4,
            },
        )
        .unwrap()
    })
}
/// Explicit owner-installed policy. No transport starts exposed implicitly.
fn policy() -> LocalPolicy {
    LocalPolicy {
        mode: ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![Transport::OwnerIpc, Transport::OwnerStdio, Transport::Http],
        http_hosts: vec!["localhost:9876".into()],
        http_origins: vec!["http://localhost:9876".into()],
    }
}
/// Checked fixture ID; IDs remain labels rather than authorization.
fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
/// Existing version-1 root browsing frame, without alternative operation or path catalog.
fn browse(library: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"test","command":{"op":"browse","library_id":library,"mount_id":null,"path":"","cursor":null,"limit":50}})).unwrap()
}
/// Actual owner-selected native filesystem fixture.
struct Fixture {
    directory: tempfile::TempDir,
    authority: NativeAuthority,
    owner: Arc<Owner>,
    selection: Selection,
    session: Session,
}
impl Fixture {
    /// Create public/hidden real files and explicitly admit direct owner-launched stdio as an agent.
    fn new() -> Self {
        account();
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("visible.txt"),
            b"current native bytes",
        )
        .unwrap();
        fs::write(directory.path().join("secret.txt"), b"hidden native bytes").unwrap();
        let (authority, owner) = NativeAuthority::new();
        let owner = Arc::new(owner);
        let selection = owner
            .select(
                id("library"),
                File::open(directory.path()).unwrap(),
                policy(),
            )
            .unwrap();
        let session = authority
            .connect(std::slice::from_ref(&selection), TransportFacts::OwnerStdio)
            .unwrap();
        Self {
            directory,
            authority,
            owner,
            selection,
            session,
        }
    }
    /// Instantiate a real native browse domain host against the exact selected directory.
    fn host(&self) -> NativeBrowse {
        NativeBrowse {
            root: self.directory.path().into(),
            calls: 0,
            before_return: None,
        }
    }
}
/// Configuration/package guards bind actual owner/selection/epoch, never equal labels.
#[test]
fn owner_selection_lease_checks_actual_permit_and_compare_advances_without_policy_reset() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert!(second.owner.pin_selection(&first.selection).is_err());
    let lease = first.owner.pin_selection(&first.selection).unwrap();
    let command = decode_request(&browse("library")).unwrap().command;
    let one = Runtime::new(first.authority.clone());
    let two = Runtime::new(second.authority.clone());
    let p = one.native_scope(&first.session, &command, None).unwrap();
    let foreign = two.native_scope(&second.session, &command, None).unwrap();
    lease.check_permit(&p).unwrap();
    assert_eq!(lease.check_permit(&foreign), Err(ErrorCode::NotExposed));
    let next = first.owner.revise_selection(&lease).unwrap();
    assert!(lease.check().is_err());
    assert!(p.check().is_err());
    assert!(first.owner.revise_selection(&lease).is_err());
    let fresh = one.native_scope(&first.session, &command, None).unwrap();
    next.check_permit(&fresh).unwrap();
    let mut excluded = policy();
    excluded
        .excluded_paths
        .push(RelativePath::new("secret.txt").unwrap());
    first.owner.set_policy(&first.selection, excluded).unwrap();
    assert!(next.check().is_err());
    first.owner.set_policy(&first.selection, policy()).unwrap();
    assert!(
        next.check().is_err(),
        "restored data cannot resurrect a native lease"
    );
}

/// Minimal actual native browse/exposure adapter used to exercise invocation authority, not safe apply.
struct NativeBrowse {
    root: PathBuf,
    calls: usize,
    before_return: Option<Box<dyn FnMut()>>,
}
impl DomainHost for NativeBrowse {
    /// Scope only the already admitted library; no source/plan identity is invented.
    fn resolve(
        &mut self,
        _: &Command,
        _: &Permit,
    ) -> std::result::Result<ResolvedScope, ErrorCode> {
        Ok(ResolvedScope {
            libraries: vec![id("library")],
            paths: vec![],
        })
    }
    /// Enumerate actual native entries only under the selected descriptor and current exclusions.
    fn execute(
        &mut self,
        domain: Domain,
        command: &Command,
        permit: &Permit,
    ) -> std::result::Result<WireResult, ErrorCode> {
        self.calls += 1;
        let result = match command {
            Command::Browse { library_id, .. } => {
                assert_eq!(domain, Domain::Storage);
                permit.with_root(library_id, |root| {
                    use std::os::unix::fs::MetadataExt;
                    let actual = root.metadata().map_err(|_| ErrorCode::Denied)?;
                    let selected = fs::metadata(&self.root).map_err(|_| ErrorCode::Denied)?;
                    if (actual.dev(), actual.ino()) != (selected.dev(), selected.ino()) {
                        return Err(ErrorCode::Changed);
                    }
                    Ok(())
                })?;
                let mut entries = Vec::new();
                for entry in fs::read_dir(&self.root).map_err(|_| ErrorCode::Denied)? {
                    let entry = entry.map_err(|_| ErrorCode::Denied)?;
                    let name = entry
                        .file_name()
                        .into_string()
                        .map_err(|_| ErrorCode::Unsupported)?;
                    let path =
                        RelativePath::new(name.clone()).map_err(|_| ErrorCode::Unsupported)?;
                    if permit.check_path(library_id, &path).is_err() {
                        continue;
                    }
                    let kind = if entry.file_type().map_err(|_| ErrorCode::Denied)?.is_dir() {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    };
                    entries.push(ListingEntry {
                        name,
                        path,
                        kind,
                        source: None,
                    });
                }
                entries.sort_by(|a, b| a.name.cmp(&b.name));
                WireResult::Listing {
                    entries,
                    next_cursor: None,
                    complete: true,
                }
            }
            Command::ExposureGet { library_id } => WireResult::Exposure {
                exposure: Box::new(permit.exposure(library_id)?),
            },
            _ => return Err(ErrorCode::Unsupported),
        };
        if let Some(hook) = self.before_return.as_mut() {
            hook();
        }
        Ok(result)
    }
}
/// Parse only the complete bounded encoded response; a partial frame is not a successful result.
fn result(delivery: &Delivery) -> serde_json::Value {
    serde_json::from_slice::<serde_json::Value>(&delivery.encode().unwrap()).unwrap()["result"]
        .clone()
}
/// Deliberate native fixture interaction; production cannot use this test implementation as consent.
struct Confirm;
impl HumanIntent for Confirm {
    /// Assert the exact owner-only intention and simulate the native user's explicit confirmation.
    fn confirm(&mut self, intention: NativeIntent<'_>) -> std::io::Result<()> {
        assert!(matches!(intention.command, Command::ExposureGet { .. }));
        Ok(())
    }
}

/// A real public file observation succeeds; exclusions are applied before names/counts are returned.
#[test]
fn native_browse_and_current_exclusions() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let mut host = fixture.host();
    let first = runtime
        .dispatch(&browse("library"), &fixture.session, &mut host)
        .unwrap();
    assert_eq!(result(&first)["entries"].as_array().unwrap().len(), 2);
    drop(first);
    let mut current = policy();
    current
        .excluded_paths
        .push(RelativePath::new("secret.txt").unwrap());
    fixture
        .owner
        .set_policy(&fixture.selection, current)
        .unwrap();
    let next = runtime
        .dispatch(&browse("library"), &fixture.session, &mut host)
        .unwrap();
    assert_eq!(result(&next)["entries"][0]["name"], "visible.txt");
    assert_eq!(result(&next)["entries"].as_array().unwrap().len(), 1);
}

/// A decoded ID cannot switch selected roots, and a separate native instance cannot borrow selections.
#[test]
fn dto_ids_and_foreign_instances_do_not_grant_roots() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let mut host = fixture.host();
    let refused = runtime
        .dispatch(&browse("unselected"), &fixture.session, &mut host)
        .unwrap();
    assert_eq!(result(&refused)["error"]["code"], "not_exposed");
    assert_eq!(host.calls, 0);
    let (foreign, _) = NativeAuthority::new();
    assert!(
        foreign
            .connect(
                std::slice::from_ref(&fixture.selection),
                TransportFacts::OwnerStdio
            )
            .is_err()
    );
}

/// Neither owner-launched stdio nor a command tuple constitutes human confirmation.
#[test]
fn agent_denial_and_exact_one_use_native_human_intention() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let bytes = br#"{"wire_version":1,"request_id":"owner","command":{"op":"exposure_get","library_id":"library"}}"#;
    let mut host = fixture.host();
    let denied = runtime
        .dispatch(bytes, &fixture.session, &mut host)
        .unwrap();
    assert_eq!(result(&denied)["error"]["code"], "owner_required");
    assert_eq!(host.calls, 0);
    let command = decode_request(bytes).unwrap().command;
    let approval = fixture
        .owner
        .approve(&fixture.session, &command, &mut Confirm)
        .unwrap();
    let allowed = runtime
        .dispatch_approved(bytes, &fixture.session, &mut host, Some(approval))
        .unwrap();
    assert_eq!(result(&allowed)["type"], "exposure");
    assert_eq!(result(&allowed)["exposure"]["mode"], "read_write");
    let replay_without_capability = runtime
        .dispatch(bytes, &fixture.session, &mut host)
        .unwrap();
    assert_eq!(
        result(&replay_without_capability)["error"]["code"],
        "owner_required"
    );
}

/// Changed policy/session/command refuses an old exact native approval before any domain effects.
#[test]
fn stale_and_mismatched_approval_refuse() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let bytes = br#"{"wire_version":1,"request_id":"owner","command":{"op":"exposure_get","library_id":"library"}}"#;
    let command = decode_request(bytes).unwrap().command;
    let approval = fixture
        .owner
        .approve(&fixture.session, &command, &mut Confirm)
        .unwrap();
    fixture
        .owner
        .set_policy(&fixture.selection, policy())
        .unwrap();
    let mut host = fixture.host();
    let refused = runtime
        .dispatch_approved(bytes, &fixture.session, &mut host, Some(approval))
        .unwrap();
    assert_eq!(result(&refused)["error"]["code"], "confirmation_required");
    assert_eq!(host.calls, 0);
    let approval = fixture
        .owner
        .approve(&fixture.session, &command, &mut Confirm)
        .unwrap();
    let refused = runtime
        .dispatch_approved(
            &browse("library"),
            &fixture.session,
            &mut host,
            Some(approval),
        )
        .unwrap();
    assert_eq!(result(&refused)["error"]["code"], "confirmation_required");
    assert_eq!(host.calls, 0);
}

/// Revocation after a real native domain observation redacts before the dispatcher returns data.
#[test]
fn revocation_during_domain_execution_stops_delivery() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let mut host = fixture.host();
    let owner = fixture.owner.clone();
    let selection = fixture.selection.clone();
    host.before_return = Some(Box::new(move || owner.revoke(&selection).unwrap()));
    let refused = runtime
        .dispatch(&browse("library"), &fixture.session, &mut host)
        .unwrap();
    assert_eq!(host.calls, 1);
    assert_eq!(result(&refused)["error"]["code"], "not_exposed");
    assert!(
        !String::from_utf8(refused.encode().unwrap())
            .unwrap()
            .contains("visible.txt")
    );
}

/// A result prepared before revocation cannot be reused as an admitted delivery afterward.
#[test]
fn revocation_after_dispatch_redacts_encoded_result() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let mut host = fixture.host();
    let delivery = runtime
        .dispatch(&browse("library"), &fixture.session, &mut host)
        .unwrap();
    fixture.owner.revoke(&fixture.selection).unwrap();
    assert_eq!(delivery.check(), Err(ErrorCode::NotExposed));
    assert_eq!(result(&delivery)["error"]["code"], "not_exposed");
}

/// HTTP requires literal actual Host/Origin and never admits sensitivity or human intentions.
#[test]
fn http_actual_facts_are_not_identity_or_human_grants() {
    let fixture = Fixture::new();
    let facts = |host: &str, origin: &str| TransportFacts::Http {
        loopback: true,
        host: host.into(),
        origin: Some(origin.into()),
    };
    assert!(
        fixture
            .authority
            .connect(
                std::slice::from_ref(&fixture.selection),
                facts("localhost:9876.evil", "http://localhost:9876")
            )
            .is_err()
    );
    assert!(
        fixture
            .authority
            .connect(
                std::slice::from_ref(&fixture.selection),
                facts("localhost:9876", "http://evil.invalid")
            )
            .is_err()
    );
    let session = fixture
        .authority
        .connect(
            std::slice::from_ref(&fixture.selection),
            facts("localhost:9876", "http://localhost:9876"),
        )
        .unwrap();
    let bytes = br#"{"wire_version":1,"request_id":"owner","command":{"op":"exposure_get","library_id":"library"}}"#;
    assert!(
        fixture
            .owner
            .approve(
                &session,
                &decode_request(bytes).unwrap().command,
                &mut Confirm
            )
            .is_err()
    );
    let mut sensitive = policy();
    sensitive.sensitive = true;
    fixture
        .owner
        .set_policy(&fixture.selection, sensitive)
        .unwrap();
    let runtime = Runtime::new(fixture.authority.clone());
    let refused = runtime
        .dispatch(&browse("library"), &session, &mut fixture.host())
        .unwrap();
    assert_eq!(result(&refused)["error"]["code"], "not_exposed");
}

/// Unknown request fields/version fail at the shared codec and do not execute a hostcall.
#[test]
fn invalid_request_refuses_before_native_effects() {
    let fixture = Fixture::new();
    let runtime = Runtime::new(fixture.authority.clone());
    let mut host = fixture.host();
    for bytes in [br#"{"wire_version":1,"request_id":"x","command":{"op":"exposure_get","library_id":"library","confirmed":true}}"#.as_slice(), br#"{"wire_version":2,"request_id":"x","command":{"op":"library_list"}}"#.as_slice()] {
        assert!(matches!(runtime.dispatch(bytes, &fixture.session, &mut host), Err(RuntimeError::Codec(_))));
    }
    assert_eq!(host.calls, 0);
}

/// Actual IPC peer observations are checked, but matching UID still cannot create a human capability.
#[test]
fn native_peer_uid_must_match() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .authority
            .connect(
                std::slice::from_ref(&fixture.selection),
                TransportFacts::OwnerIpc {
                    peer_uid: 1,
                    service_uid: 2
                }
            )
            .is_err()
    );
}

/// Stream capacity is service-wide, read-admitted, bounded and independent from underlying job cancellation.
#[test]
fn status_stream_capacity_and_no_other_operations() {
    let fixture = Fixture::new();
    let facts = TransportFacts::Http {
        loopback: true,
        host: "localhost:9876".into(),
        origin: None,
    };
    let session = fixture
        .authority
        .connect(std::slice::from_ref(&fixture.selection), facts)
        .unwrap();
    let runtime = Runtime::new(fixture.authority.clone());
    assert!(matches!(
        runtime.status_stream(&browse("library"), &session),
        Err(ErrorCode::InvalidRequest)
    ));
    let bytes =
        br#"{"wire_version":1,"request_id":"stream","command":{"op":"job_status","job_id":"job"}}"#;
    let streams: Vec<_> = (0..16)
        .map(|_| runtime.status_stream(bytes, &session).unwrap())
        .collect();
    assert!(matches!(
        runtime.status_stream(bytes, &session),
        Err(ErrorCode::BudgetExhausted)
    ));
    drop(streams);
    let mut stream = runtime.status_stream(bytes, &session).unwrap();
    let mut host = fixture.host();
    let error = stream.next(&mut host).unwrap().unwrap();
    assert!(error.is_error());
    assert!(stream.ended());
    assert!(stream.next(&mut host).unwrap().is_none());
}
