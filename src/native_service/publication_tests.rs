//! Actual native EPUB intent, prepaid publication, no-clobber installation, and owning recovery.
//! Synthetic channel interaction exercises native approval; it is not human/platform acceptance.
use super::*;
use crate::native_service::embedding_tests::{digest, ready};
use knowledge_core::admission::{Account, Waiting};
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::{Claim, Key, Priority, Reservation};
use knowledge_retrieval::Work;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use wire::authority::{HumanApproval, HumanIntent, NativeIntent};

const TEST_NAME: &str =
    "native_service::publication_tests::actual_epub_native_intent_publication_and_owning_recovery";
const FIRST: &str = "# First\nNative publication first chapter Ω\n";
const SECOND: &str = "# Second\nNative publication second chapter Ω\n";
const FINAL: &str = "selected.epub";

/// Present the entire actual command over an owned native channel and wait for its response.
struct ChannelIntent(UnixStream);
impl HumanIntent for ChannelIntent {
    /// A real read/write exchange must finish; no request flag or unconditional callback approves.
    fn confirm(&mut self, intent: NativeIntent<'_>) -> std::io::Result<()> {
        assert!(matches!(intent.transport, TransportFacts::OwnerStdio));
        let bytes = serde_json::to_vec(intent.command).map_err(std::io::Error::other)?;
        self.0.write_all(&(bytes.len() as u32).to_le_bytes())?;
        self.0.write_all(&bytes)?;
        self.0.flush()?;
        let mut answer = [0];
        self.0.read_exact(&mut answer)?;
        if answer == [1] {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "synthetic native owner declined",
            ))
        }
    }
}

/// Drive actual native intention I/O, checking the complete displayed command before response.
pub(super) fn approve(
    owner: &Owner,
    session: &Session,
    command: &wire::Command,
    accept: bool,
) -> Result<HumanApproval, wire::ErrorCode> {
    let (front, mut driver) = UnixStream::pair().unwrap();
    let expected = serde_json::to_value(command).unwrap();
    let interaction = std::thread::spawn(move || {
        let mut length = [0; 4];
        driver.read_exact(&mut length).unwrap();
        let length = u32::from_le_bytes(length) as usize;
        assert!(length <= 1024 * 1024);
        let mut bytes = vec![0; length];
        driver.read_exact(&mut bytes).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            expected,
            "native channel presents the complete exact effect selection before approval"
        );
        driver.write_all(&[u8::from(accept)]).unwrap();
        driver.flush().unwrap();
    });
    let result = owner.approve(session, command, &mut ChannelIntent(front));
    interaction.join().unwrap();
    result
}

/// Admit the exact owner-approved command through the real native runtime.
fn approved(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    command: &wire::Command,
) -> Permit {
    service
        .native_scope(
            session,
            command,
            Some(approve(owner, session, command, true).unwrap()),
        )
        .unwrap()
}

/// Use the existing wire decoder, rather than constructing a wider test-only operation.
fn command(value: serde_json::Value) -> wire::Command {
    wire::decode_request(
        &serde_json::to_vec(&serde_json::json!({
            "wire_version":1,"request_id":"native-publication","command":value
        }))
        .unwrap(),
    )
    .unwrap()
    .command
}

/// Fixed real source verification and complete plan work bounds for the small synthetic publication.
fn budget() -> Budget {
    Budget {
        max_files: 8,
        max_bytes: 16 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 256 * 1024 * 1024,
    }
}

/// Explicit current policy; an owner confirmation still cannot bypass a later revision/revocation.
fn policy() -> LocalPolicy {
    LocalPolicy {
        mode: wire::ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    }
}

/// Select an independently held destination root with actual descriptor/name currentness checks.
fn destination(owner: &Owner, library: wire::Id, path: &std::path::Path) -> Selection {
    let path = path.to_owned();
    let observed = fs::symlink_metadata(&path).unwrap();
    let identity = (observed.dev(), observed.ino());
    owner
        .select_current(
            library,
            File::open(&path).unwrap(),
            policy(),
            Arc::new(move |held| {
                fs::symlink_metadata(&path)
                    .ok()
                    .zip(held.metadata().ok())
                    .is_some_and(|(named, held)| {
                        named.is_dir()
                            && !named.file_type().is_symlink()
                            && held.is_dir()
                            && (named.dev(), named.ino()) == identity
                            && (held.dev(), held.ino()) == identity
                    })
            }),
        )
        .unwrap()
}

/// Open the exact admitted source descriptor; the hint remains recipe/link metadata only.
fn source(service: &NativeLibraryService, permit: &Permit, path: &str) -> NativeSource {
    service
        .source(permit, &wire::RelativePath::new(path).unwrap(), budget())
        .unwrap()
}

/// Keep full cancellation identity data while the actual affine Claim moves into the native owner.
struct JobKey {
    context: String,
    library: String,
    source: String,
    version: ContentDigest,
    recipe: ContentDigest,
    processor: ContentDigest,
}
impl JobKey {
    /// Record the actual ordinary-local submitted key without constructing a work capability.
    fn capture(key: Key<'_>) -> Self {
        assert!(key.mount.is_none());
        Self {
            context: key.context.into(),
            library: key.library.into(),
            source: key.source.into(),
            version: key.version.clone(),
            recipe: key.recipe.clone(),
            processor: key.processor.clone(),
        }
    }
    /// Borrow the exact queue cancellation qualifier; the real Claim remains separately owned.
    fn key(&self) -> Key<'_> {
        Key {
            context: &self.context,
            library: &self.library,
            mount: None,
            source: &self.source,
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }
}

/// Inspect the installed EPUB with an independent standard ZIP/XML reader and its declared spine.
fn assert_epub(path: &std::path::Path) {
    let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    /// Read one declared EPUB entry within a fixed fixture bound.
    fn text(archive: &mut zip::ZipArchive<File>, name: &str) -> String {
        let mut value = String::new();
        archive
            .by_name(name)
            .unwrap()
            .take(1024 * 1024)
            .read_to_string(&mut value)
            .unwrap();
        value
    }
    assert_eq!(text(&mut archive, "mimetype"), "application/epub+zip");
    let container = text(&mut archive, "META-INF/container.xml");
    let container = roxmltree::Document::parse(&container).unwrap();
    let package = container
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .unwrap()
        .attribute("full-path")
        .unwrap();
    let opf = text(&mut archive, package);
    let opf = roxmltree::Document::parse(&opf).unwrap();
    let chapters: Vec<_> = opf
        .descendants()
        .filter(|node| node.has_tag_name("itemref"))
        .map(|node| {
            let id = node.attribute("idref").unwrap();
            let href = opf
                .descendants()
                .find(|node| node.has_tag_name("item") && node.attribute("id") == Some(id))
                .unwrap()
                .attribute("href")
                .unwrap();
            text(
                &mut archive,
                std::path::Path::new(package)
                    .parent()
                    .unwrap()
                    .join(href)
                    .to_str()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(
        chapters.len(),
        2,
        "the installed declared spine contains exactly the approved pages"
    );
    assert!(chapters[0].contains("Native publication first chapter Ω"));
    assert!(chapters[1].contains("Native publication second chapter Ω"));
}

/// Locate the already built real parser used by the normal native producer, before fault wrapping.
#[cfg(target_os = "linux")]
fn parser_worker() -> std::path::PathBuf {
    for variable in [
        "CARGO_BIN_EXE_parser_worker",
        "KNOWLEDGE_PROCESSING_PARSER_WORKER",
    ] {
        if let Some(path) = std::env::var_os(variable) {
            let path = std::path::PathBuf::from(path);
            assert!(path.is_file());
            return path;
        }
    }
    let exe = std::env::current_exe().unwrap();
    let path = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("parser-worker");
    assert!(
        path.is_file(),
        "the actual parser-worker must be built; no fake encoder is substituted"
    );
    path
}

/// Observe only this owned fixture's exact native process start and state, never commands/environment.
#[cfg(target_os = "linux")]
fn process_state(pid: u32) -> Option<(u64, char)> {
    let bytes = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = bytes.rsplit_once(')')?.1.split_whitespace().collect();
    Some((
        fields.get(19)?.parse().ok()?,
        fields.first()?.chars().next()?,
    ))
}

/// Exact escaped pipe holder, stopped before a failing assertion can drop its foreground native owner.
#[cfg(target_os = "linux")]
struct EscapedPipe {
    pid: u32,
    start: u64,
    stopped: bool,
}
#[cfg(target_os = "linux")]
impl EscapedPipe {
    /// Read a task-owned native PID/start receipt and independently qualify its still-live process.
    fn read() -> Self {
        let path = std::env::var_os("OMNI_PUBLICATION_ESCAPE_MARKER").unwrap();
        let receipt: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let pid = u32::try_from(receipt["pid"].as_u64().unwrap()).unwrap();
        let start = receipt["start"].as_u64().unwrap();
        let observed = process_state(pid)
            .expect("escaped fixture remains native-live while its pipes are unknown");
        assert_eq!(observed.0, start);
        assert!(!matches!(observed.1, 'Z' | 'X'));
        Self {
            pid,
            start,
            stopped: false,
        }
    }
    /// Stop only the captured native identity; PID exit alone never settles the processing owner.
    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        if let Some((start, state)) = process_state(self.pid) {
            anyhow::ensure!(
                start == self.start,
                "escaped fixture native identity changed; refuse signal"
            );
            if !matches!(state, 'Z' | 'X') {
                let pid = rustix::process::Pid::from_raw(i32::try_from(self.pid)?)
                    .ok_or_else(|| anyhow::anyhow!("invalid task-owned escaped PID"))?;
                rustix::process::kill_process(pid, rustix::process::Signal::KILL)?;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while process_state(self.pid)
            .is_some_and(|(start, state)| start == self.start && !matches!(state, 'Z' | 'X'))
        {
            anyhow::ensure!(
                Instant::now() < deadline,
                "exact escaped fixture remains live"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        self.stopped = true;
        Ok(())
    }
}
#[cfg(target_os = "linux")]
impl Drop for EscapedPipe {
    /// Bound cleanup to the task-owned process even if a credit/collection assertion fails.
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Same actual owner process is required for meaningful complete-credit and native settlement assertions.
#[test]
fn actual_epub_native_intent_publication_and_owning_recovery() {
    let Some(mode) = std::env::var_os("OMNI_PUBLICATION_MODE") else {
        let modes = vec![
            "normal",
            "no-approval",
            "declined-approval",
            "wrong-source-intent",
            "wrong-anchor-intent",
            "wrong-name",
            "wrong-order",
            "wrong-plan",
            "foreign-selection",
            "foreign-owner",
            "source-changed",
            "destination-revoked",
            "destination-replaced",
            "collision",
            "waiting",
            "pressure",
            "processing-pressure",
            "cancelled",
            "installed-revoked",
        ];
        #[cfg(target_os = "linux")]
        let modes = {
            let mut modes = modes;
            modes.push("unknown-io");
            modes
        };
        for mode in modes {
            let cache = tempfile::tempdir().unwrap();
            fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
            #[cfg(target_os = "linux")]
            let faults = tempfile::tempdir().unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env("OMNI_PUBLICATION_MODE", mode)
                .env("OMNI_CACHE_DIR", cache.path());
            #[cfg(target_os = "linux")]
            if mode == "unknown-io" {
                let wrapper = faults.path().join("native-pipe-fault.py");
                fs::write(&wrapper, r#"#!/usr/bin/python3
import json, os, subprocess, sys
real = os.environ['OMNI_PUBLICATION_REAL_WORKER']
if len(sys.argv) < 2 or sys.argv[1] != 'export-epub-held':
    os.execv(real, [real, *sys.argv[1:]])
payload = sys.stdin.buffer.read()
done = subprocess.run([real, *sys.argv[1:]], input=payload, stdout=sys.stdout, stderr=sys.stderr, close_fds=False)
if done.returncode != 0:
    sys.exit(done.returncode)
p = subprocess.Popen(['/bin/sleep', '60'], start_new_session=True, stdout=sys.stdout, stderr=sys.stderr, close_fds=True)
with open('/proc/%d/stat' % p.pid) as f:
    start = int(f.read().rsplit(')', 1)[1].split()[19])
with open(os.environ['OMNI_PUBLICATION_ESCAPE_MARKER'], 'w') as f:
    json.dump({'pid': p.pid, 'start': start}, f)
sys.exit(0)
"#).unwrap();
                fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
                child
                    .env("OMNI_PUBLICATION_REAL_WORKER", parser_worker())
                    .env(
                        "OMNI_PUBLICATION_ESCAPE_MARKER",
                        faults.path().join("escaped.json"),
                    )
                    .env("CARGO_BIN_EXE_parser_worker", &wrapper)
                    .env("KNOWLEDGE_PROCESSING_PARSER_WORKER", &wrapper);
            }
            let output = child.output().unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "actual native publication {mode}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let mode = mode.to_str().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path().join("home");
    let output = fixture.path().join("selected-output");
    fs::create_dir(&output).unwrap();
    let kb = KnowledgeBase::init(&home, "Synthetic native publication").unwrap();
    fs::write(home.join("wiki/first.md"), FIRST).unwrap();
    fs::write(home.join("wiki/second.md"), SECOND).unwrap();
    let (service, owner, _source_selection) =
        NativeLibraryService::ordinary_local(kb, policy()).unwrap();
    let owner = Arc::new(owner);
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let selected = destination(&owner, service.library_id().clone(), &output);
    let destination_session = service
        .connect_selected(&selected, TransportFacts::OwnerStdio)
        .unwrap();
    let plan_command = command(
        serde_json::json!({"op":"epub_plan","library_id":service.library_id(),
        "page_paths":["wiki/first.md","wiki/second.md"],"media_source_ids":[],"destination":FINAL,
        "budget":{"max_files":8,"max_bytes":"16777216","max_millis":60000,"max_memory_bytes":"268435456"}}),
    );
    if mode == "no-approval" {
        assert!(matches!(
            service.native_scope(&destination_session, &plan_command, None),
            Err(wire::ErrorCode::OwnerRequired)
        ));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        return;
    }
    if mode == "declined-approval" {
        assert!(matches!(
            approve(&owner, &destination_session, &plan_command, false),
            Err(wire::ErrorCode::ConfirmationRequired)
        ));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        return;
    }
    let source_plan_permit = approved(&service, &owner, &session, &plan_command);
    let destination_plan_permit = approved(&service, &owner, &destination_session, &plan_command);
    let status =
        command(serde_json::json!({"op":"library_status","library_id":service.library_id()}));
    let status_permit = service.native_scope(&session, &status, None).unwrap();
    let member_permit = if mode == "wrong-source-intent" {
        &status_permit
    } else {
        &source_plan_permit
    };
    let mut members = vec![
        NativeEpubMember {
            source: source(&service, member_permit, "wiki/first.md"),
            name: RelativePath::new("first.md").unwrap(),
        },
        NativeEpubMember {
            source: source(&service, member_permit, "wiki/second.md"),
            name: RelativePath::new("second.md").unwrap(),
        },
    ];
    if mode == "wrong-order" {
        members.reverse();
    }
    let recipe = NativeEpubPlan {
        title: "Actually approved native book".into(),
        include_media: false,
        child_rss_bytes: None,
        storage_bytes: 1024 * 1024,
    };
    let final_name = if mode == "wrong-name" {
        "unapproved.epub"
    } else {
        FINAL
    };
    let metadata =
        ready(NativePublicationPlan::metadata_requirement(&members, &recipe, final_name).unwrap());
    if mode == "foreign-owner" {
        let (_, other) = wire::authority::NativeAuthority::new();
        assert!(
            NativePublicationPlan::epub(
                &other,
                &selected,
                destination_plan_permit,
                final_name,
                members,
                recipe,
                metadata
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        return;
    }
    let planned = NativePublicationPlan::epub(
        &owner,
        &selected,
        destination_plan_permit,
        final_name,
        members,
        recipe,
        metadata,
    );
    if matches!(mode, "wrong-source-intent" | "wrong-name" | "wrong-order") {
        assert!(
            planned.is_err(),
            "wrong actual source command/name/order cannot construct publication authority"
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        return;
    }
    let mut plan = planned.unwrap();
    let plan_id = if mode == "wrong-plan" {
        wire::Id::new("foreign-plan").unwrap()
    } else {
        plan.id().clone()
    };
    let export_command = command(
        serde_json::json!({"op":"epub_export","library_id":service.library_id(), "plan_id":plan_id,"operation_id":"native-publication"}),
    );
    let foreign_output = fixture.path().join("foreign-output");
    let export_permit = if mode == "foreign-selection" {
        fs::create_dir(&foreign_output).unwrap();
        let foreign = destination(&owner, service.library_id().clone(), &foreign_output);
        let session = service
            .connect_selected(&foreign, TransportFacts::OwnerStdio)
            .unwrap();
        approved(&service, &owner, &session, &export_command)
    } else {
        approved(&service, &owner, &destination_session, &export_command)
    };
    if mode == "installed-revoked" {
        let owner = owner.clone();
        let selected = selected.clone();
        plan.test_after_install(move || owner.revoke(&selected).unwrap());
        plan.test_install_fault();
    }
    let anchor_permit = if mode == "wrong-anchor-intent" {
        &status_permit
    } else {
        &source_plan_permit
    };
    let processing = source(&service, anchor_permit, "wiki/first.md")
        .processing(
            plan.recipe().clone(),
            digest(include_bytes!("publication_tests.rs")),
        )
        .unwrap();
    let need = plan
        .requirement(Reservation {
            memory_bytes: 64 * 1024 * 1024,
            temporary_bytes: 1024 * 1024,
        })
        .unwrap();
    let key = JobKey::capture(processing.key());
    let account = Account::current().unwrap();
    let waiting_pressure = if mode == "waiting" {
        Some(ready(Reservation {
            memory_bytes: account.status().unwrap().memory_available,
            temporary_bytes: 0,
        }))
    } else {
        None
    };
    let mut actual_budget = budget();
    if mode == "unknown-io" {
        actual_budget.max_millis = 1000;
    }
    let submitted = processing
        .submit(NativeJobPlan {
            operation_id: "native-publication",
            job_id: "native-publication",
            budget: actual_budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    if mode == "waiting" {
        assert!(account.queue().claim(Instant::now()).unwrap().is_none());
        assert_eq!(
            account
                .job_status(submitted.job, key.key())
                .unwrap()
                .waiting,
            Some(Waiting::Memory)
        );
        assert_eq!(
            fs::read_dir(&output).unwrap().count(),
            0,
            "waiting admission precedes every publication effect"
        );
    }
    drop(waiting_pressure);
    let claim: Claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), submitted.job);
    let processing_pressure = if mode == "processing-pressure" {
        let pressure = ready(Reservation {
            memory_bytes: account.status().unwrap().memory_available,
            temporary_bytes: 0,
        });
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "the complete admitted native job starts with no unrelated global memory available"
        );
        Some(pressure)
    } else {
        None
    };
    let built = processing.export_epub_planned(claim, plan, export_permit);
    #[cfg(target_os = "linux")]
    if mode == "unknown-io" {
        let escaped_guard = EscapedPipe::read();
        let mut error = match built {
            Err(error) => error,
            Ok(_) => {
                panic!("actual escaped open pipes cannot become a successful native EPUB result")
            }
        };
        let mut escaped = escaped_guard;
        assert!(
            error
                .cause()
                .is::<knowledge_processing::worker::UnsettledChild>(),
            "the actual native group/pipe owner must remain typed and reachable: {:#}",
            error.cause()
        );
        let before = account.status().unwrap().reserved;
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        assert!(
            before.temporary_bytes >= 2 * 1024 * 1024,
            "actual unknown worker and prepaid publication sibling remain charged together"
        );
        assert!(
            error.collect().is_err(),
            "a leader exit or PID wait cannot stand in for actual escaped-pipe EOF"
        );
        assert_eq!(
            account.status().unwrap().reserved,
            before,
            "unknown native collection cannot release sibling/worker/output credit"
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        escaped.stop().unwrap();
        error.collect().unwrap_or_else(|cause| {
            panic!("actual group/pipe collection and exact held-output reconciliation: {cause:#}")
        });
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "native collection is distinct from Claim settlement"
        );
        assert_eq!(
            fs::read_dir(&output).unwrap().count(),
            0,
            "failed native production never installs a final artifact"
        );
        drop(error);
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
        assert_eq!(
            fs::read_to_string(home.join("wiki/first.md")).unwrap(),
            FIRST
        );
        assert_eq!(
            fs::read_to_string(home.join("wiki/second.md")).unwrap(),
            SECOND
        );
        return;
    }
    if matches!(
        mode,
        "wrong-anchor-intent" | "wrong-plan" | "foreign-selection"
    ) {
        let error = match built {
            Err(error) => error,
            Ok(_) => panic!(
                "wrong actual anchor/plan/selection must refuse before processing publication"
            ),
        };
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        if foreign_output.exists() {
            assert_eq!(fs::read_dir(&foreign_output).unwrap().count(), 0);
        }
        drop(error);
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
        return;
    }
    let book =
        built.unwrap_or_else(|error| panic!("actual planned EPUB producer: {:#}", error.cause()));
    assert_eq!(book.value().unwrap().chapters, 2);
    assert_eq!(account.status().unwrap().queue.unsettled, 1);
    assert_eq!(
        fs::read_dir(&output).unwrap().count(),
        0,
        "complete private processing is not destination publication"
    );
    if mode == "source-changed" {
        fs::write(home.join("wiki/second.md"), "concurrent original edit").unwrap();
    }
    if mode == "destination-revoked" {
        owner.revoke(&selected).unwrap();
    }
    if mode == "destination-replaced" {
        fs::rename(&output, fixture.path().join("moved-output")).unwrap();
        fs::create_dir(&output).unwrap();
    }
    if mode == "collision" {
        fs::write(output.join(FINAL), b"competing native file").unwrap();
    }
    if mode == "cancelled" {
        account.queue().cancel(submitted.job, key.key()).unwrap();
    }
    let pressure: Option<Work> = if mode == "pressure" {
        let status = account.status().unwrap();
        Some(ready(Reservation {
            memory_bytes: status.memory_available,
            temporary_bytes: status.storage_available,
        }))
    } else {
        None
    };
    if mode == "pressure" {
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "publication starts under actual complete account pressure with its sibling already prepaid"
        );
    }
    let published = book.publish();
    if matches!(
        mode,
        "source-changed"
            | "destination-revoked"
            | "destination-replaced"
            | "collision"
            | "cancelled"
            | "installed-revoked"
    ) {
        let mut error = match published {
            Err(error) => error,
            Ok(_) => panic!(
                "changed/cancelled/uncertain publication must return its actual owning phase"
            ),
        };
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "cancel/error is not native write collection"
        );
        error.collect().unwrap();
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "collection alone does not settle the actual Claim"
        );
        if mode == "installed-revoked" {
            assert_epub(&output.join(FINAL));
        } else if mode == "collision" {
            assert_eq!(
                fs::read(output.join(FINAL)).unwrap(),
                b"competing native file"
            );
        } else {
            assert!(!output.join(FINAL).exists());
        }
        drop(error);
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
        if mode == "installed-revoked" {
            assert_epub(&output.join(FINAL));
        }
        if mode == "source-changed" {
            assert_eq!(
                fs::read_to_string(home.join("wiki/second.md")).unwrap(),
                "concurrent original edit"
            );
        }
        if mode == "destination-replaced" {
            assert_eq!(
                fs::read_dir(fixture.path().join("moved-output"))
                    .unwrap()
                    .count(),
                0
            );
        }
        return;
    }
    let published = published
        .unwrap_or_else(|error| panic!("actual native EPUB publication: {:#}", error.cause()));
    assert_epub(&output.join(FINAL));
    assert_eq!(
        fs::read_dir(&output).unwrap().count(),
        1,
        "only the complete no-clobber final artifact remains"
    );
    let source = published
        .complete()
        .unwrap_or_else(|error| panic!("actual native EPUB completion: {:#}", error.cause()));
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    source.check().unwrap();
    drop(source);
    drop(pressure);
    drop(processing_pressure);
    assert_eq!(
        fs::read_to_string(home.join("wiki/first.md")).unwrap(),
        FIRST
    );
    assert_eq!(
        fs::read_to_string(home.join("wiki/second.md")).unwrap(),
        SECOND
    );
}
