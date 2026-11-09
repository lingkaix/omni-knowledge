//! Contract controls for actual tuple publication, foreground recovery, and writer contention.
//! The real producer, Core Claim, selector, cold reader, and original descriptor remain composed.
use super::*;
use std::os::unix::fs::{FileExt, PermissionsExt};

const TEST_NAME: &str = "native_service::inventory::tests::activation::actual_tuple_activation_recovery_and_writer_contention";
const BODY: &str = "# Activation fixture\nalpha survives native restart\n";

/// Copy only cancellation identity data; this value never substitutes for the actual live Claim.
struct CancelKey {
    context: String,
    library: String,
    source: String,
    version: knowledge_core::data::ContentDigest,
    recipe: knowledge_core::data::ContentDigest,
    processor: knowledge_core::data::ContentDigest,
}
impl CancelKey {
    /// Retain the actual submitted ordinary-local key for the queue's cancellation operation.
    fn capture(key: knowledge_core::jobs::Key<'_>) -> Self {
        assert!(key.mount.is_none());
        Self {
            context: key.context.to_owned(),
            library: key.library.to_owned(),
            source: key.source.to_owned(),
            version: key.version.clone(),
            recipe: key.recipe.clone(),
            processor: key.processor.clone(),
        }
    }
    /// Borrow the same actual job identity; no new authority is constructed.
    fn key(&self) -> knowledge_core::jobs::Key<'_> {
        knowledge_core::jobs::Key {
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

/// Admit an actual full-hash Refresh with the complete corpus fixture budget.
fn refresh_permit(
    service: &NativeLibraryService,
    session: &Session,
    id: &str,
    embeddings: bool,
) -> Permit {
    let command = wire::decode_request(&serde_json::to_vec(&serde_json::json!({
        "wire_version":1,"request_id":"activation-refresh","command":{
            "op":"refresh","library_id":service.library_id().as_str(),"mount_ids":[],
            "verification":"full_hash","embeddings":embeddings,"operation_id":id,
            "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":120000,"max_memory_bytes":"536870912"}
        }
    })).unwrap()).unwrap().command;
    service.native_scope(session, &command, None).unwrap()
}

/// Produce a complete sealed tuple through the existing real M1/M3/kernel owners.
fn prepared(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    id: &str,
    model: Option<NativeEmbeddingSelection>,
) -> (NativeCorpusDraft, u64, CancelKey) {
    let permit = refresh_permit(service, session, id, model.is_some());
    let inventory = reconcile(service, owner, permit.clone());
    knowledge_retrieval::KernelAdapter::initialize_runtime().unwrap();
    let job = service
        .prepare_refresh(
            owner,
            permit,
            inventory,
            model.clone(),
            holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0),
        )
        .unwrap();
    let key = CancelKey::capture(job.key());
    let need = job.corpus_requirement().unwrap();
    job.submit(NativeJobPlan {
        operation_id: id,
        job_id: id,
        budget: Budget {
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            max_memory_bytes: 512 * 1024 * 1024,
        },
        reservation: Reservation {
            memory_bytes: 256 * 1024 * 1024,
            temporary_bytes: need.temporary_bytes,
        },
        priority: knowledge_core::jobs::Priority::Normal,
        max_attempts: 1,
    })
    .unwrap();
    let claim = Account::current()
        .unwrap()
        .queue()
        .claim(Instant::now())
        .unwrap()
        .unwrap();
    let number = claim.job();
    let draft = job
        .begin_corpus(claim, admitted::generated_path().unwrap())
        .unwrap()
        .build_lexical(|_| {})
        .unwrap_or_else(|e| panic!("actual lexical preparation: {:#}", e.cause()));
    let draft = if model.is_some() {
        draft
            .build_vectors(|_| {})
            .unwrap_or_else(|e| panic!("actual vector preparation: {:#}", e.cause()))
    } else {
        draft
    };
    let draft = draft
        .prepare_tuple(|_| {})
        .unwrap_or_else(|e| panic!("actual tuple preparation: {:#}", e.cause()));
    (draft, number, key)
}

/// End a successful real draft only after its owned native consumers have been destroyed.
fn complete(draft: NativeCorpusDraft) {
    let (job, claim) = draft
        .into_job()
        .unwrap_or_else(|e| panic!("actual draft collection: {:#}", e.cause()));
    Account::current()
        .unwrap()
        .queue()
        .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Completed)
        .unwrap();
    drop(job);
    assert_eq!(
        Account::current()
            .unwrap()
            .status()
            .unwrap()
            .queue
            .unsettled,
        0
    );
}

/// Locate only the synthetic fixture's sole generated namespace for an independent filesystem actor.
fn namespace() -> std::path::PathBuf {
    let paths: Vec<_> = fs::read_dir(admitted::generated_path().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("native-tuples-")
        })
        .collect();
    assert_eq!(
        paths.len(),
        1,
        "each child owns one synthetic library and cache"
    );
    paths.into_iter().next().unwrap()
}

/// Observe persisted pointer data independently; it supplies no library or read permission.
fn current_stage() -> String {
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(namespace().join("CURRENT")).unwrap()).unwrap();
    pointer["stage"].as_str().unwrap().to_owned()
}

/// Record fixture bytes and entry kinds without following links, to detect any partial unknown cleanup.
fn tree_signature(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, String> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, String>,
    ) {
        for entry in fs::read_dir(at).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let value = if metadata.file_type().is_symlink() {
                format!("symlink:{}", fs::read_link(&path).unwrap().display())
            } else if metadata.is_dir() {
                walk(root, &path, out);
                "directory".to_owned()
            } else {
                assert!(
                    metadata.is_file(),
                    "synthetic fixture contains only regular files and explicit links"
                );
                hex::encode(Sha256::digest(fs::read(&path).unwrap()))
            };
            out.insert(path.strip_prefix(root).unwrap().to_owned(), value);
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Open a new actual Query owner and read an original through its current M1 and tuple association.
fn open_reader(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
) -> (NativeTupleRead, Permit) {
    let command = wire::decode_request(&serde_json::to_vec(&serde_json::json!({
        "wire_version":1,"request_id":"activation-read","command":{"op":"query","query":{
            "library_ids":[service.library_id().as_str()],"keywords":["alpha"],"methods":["literal"],
            "required_tags":[],"case_sensitive":false,"limit":10,"image_query":null,
            "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"}
        }}
    })).unwrap()).unwrap().command;
    let permit = service.native_scope(session, &command, None).unwrap();
    let reader = service
        .open_corpus_tuple(owner, permit.clone(), ready(NativeTupleRead::requirement()))
        .unwrap();
    assert_eq!(
        reader.test_unpinned_header_opens(),
        0,
        "search: aggregate pin precedes component opens"
    );
    assert!(
        !reader.test_aggregate_exclusive().unwrap(),
        "storage: the real aggregate reader pin remains live"
    );
    (reader, permit)
}

/// Bind the fixture original through the real tuple and current M1 owner; its pin can outlive the reader.
fn bound_original(
    service: &NativeLibraryService,
    owner: &Owner,
    reader: &NativeTupleRead,
    permit: &Permit,
) -> NativeSource {
    let header = reader.test_header().unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 0);
    let inventory = service
        .inventory_for_query(
            owner,
            permit,
            "wiki",
            header["scan"].as_str().unwrap(),
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &scratch,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
        .unwrap();
    let (offset, _) = reader.test_source_partition("raw/activation.md").unwrap();
    let (source, _) = reader
        .current_source_at(
            service,
            permit,
            observed(&inventory, "raw/activation.md"),
            offset,
        )
        .unwrap()
        .unwrap();
    drop(inventory);
    scratch.finish().unwrap();
    source
}

/// Read exact current original bytes after creating a fresh native Query.
fn read_selected(service: &NativeLibraryService, owner: &Owner, session: &Session, expected: &str) {
    let (reader, permit) = open_reader(service, owner, session);
    assert_eq!(reader.generation().unwrap(), expected);
    let source = bound_original(service, owner, &reader, &permit);
    source.check().unwrap();
    let mut bytes = vec![0; BODY.len()];
    source.file().unwrap().read_exact_at(&mut bytes, 0).unwrap();
    source.check().unwrap();
    assert_eq!(
        bytes,
        BODY.as_bytes(),
        "actual current original FD remains readable after native restart"
    );
    drop(source);
    drop(reader);
    drop(permit);
}

/// Run the real cold whole-query controller against the startup/watch-selected tuple for each method.
fn cold_query_selected(service: &NativeLibraryService, owner: &Owner, session: &Session) {
    let account = Account::current().unwrap();
    for (method, keyword) in [
        ("bm25", "alpha"),
        ("literal", "alpha survives native restart"),
        ("regex", "^alpha.*restart$"),
        ("files", "activation"),
    ] {
        let command = wire::decode_request(&serde_json::to_vec(&serde_json::json!({
            "wire_version":1,"request_id":"cold-startup-query","command":{"op":"query","query":{
                "library_ids":[service.library_id().as_str()],"keywords":[keyword],"methods":[method],
                "required_tags":[],"case_sensitive":false,"limit":10,"image_query":null,
                "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"}
            }}
        })).unwrap()).unwrap().command;
        let permit = service.native_scope(session, &command, None).unwrap();
        let reader = service
            .open_corpus_tuple(owner, permit.clone(), ready(NativeTupleRead::requirement()))
            .unwrap();
        let scan = reader.test_header().unwrap()["scan"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut scratch = holding(4 * 1024 * 1024, 0);
        let inventory = service
            .inventory_for_query(
                owner,
                &permit,
                "wiki",
                &scan,
                holding(2 * 1024 * 1024, 0),
                &Preflight {
                    holding: &scratch,
                    budget: Budget {
                        max_files: 1000,
                        max_bytes: 8 * 1024 * 1024,
                        max_millis: 60000,
                        max_memory_bytes: 4 * 1024 * 1024,
                    },
                },
            )
            .unwrap();
        let query = NativeTupleQuery::prepare(
            vec![NativeQueryLibrary::new(reader, inventory, None)],
            ready(NativeTupleQuery::metadata_requirement()),
        )
        .unwrap();
        let need = query.requirement_all().unwrap();
        let wire::Command::Query { query: admitted } = &command else {
            unreachable!()
        };
        assert!(
            need.memory_bytes <= 512 * 1024 * 1024,
            "cold fixture keeps its original512MiB ceiling"
        );
        let submitted = query
            .submit(NativeJobPlan {
                operation_id: method,
                job_id: method,
                budget: Budget {
                    max_files: admitted.budget.max_files.get(),
                    max_bytes: admitted.budget.max_bytes.get(),
                    max_millis: admitted.budget.max_millis.get(),
                    max_memory_bytes: need.memory_bytes,
                },
                reservation: need,
                priority: knowledge_core::jobs::Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
        assert_eq!(claim.job(), submitted.job);
        let controller = query.start_controller(claim.clone()).unwrap();
        assert!(
            controller.failure().is_none(),
            "cold {method} controller: {:#}",
            controller.failure().unwrap()
        );
        let result = controller.search_result().unwrap();
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(&group.library_id, service.library_id());
        assert_eq!(group.generation_id.as_ref().unwrap().as_str(), scan);
        assert_eq!(group.coverage.enumeration, wire::Enumeration::Complete);
        assert_eq!(group.coverage.unsupported.get(), 0);
        assert_eq!(
            group.files.len(),
            1,
            "cold {method} must find the actual original"
        );
        assert_eq!(group.files[0].source.path.as_str(), "raw/activation.md");
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        let (returned, alias) = controller
            .finish()
            .unwrap_or_else(|_| panic!("cold {method} native consumers remained live"));
        drop((returned, alias));
        let claim = Arc::try_unwrap(claim)
            .unwrap_or_else(|_| panic!("cold {method} retained its actual Claim"));
        account
            .queue()
            .settle(
                &claim,
                query.key(),
                knowledge_core::jobs::Outcome::Completed,
            )
            .unwrap();
        drop(query);
        scratch.finish().unwrap();
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
    }
}

/// Run each scenario with its own actual process-wide Core account and private synthetic cache.
fn child_command(
    mode: &str,
    home: &std::path::Path,
    cache: &std::path::Path,
    receipts: &std::path::Path,
) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env("OMNI_ACTIVATION_MODE", mode)
        .env("OMNI_ACTIVATION_HOME", home)
        .env("OMNI_ACTIVATION_RECEIPTS", receipts)
        .env("OMNI_CACHE_DIR", cache);
    command
}

/// Collect a normal fixture child and retain its deciding assertion diagnostics.
fn child(mode: &str, home: &std::path::Path, cache: &std::path::Path, receipts: &std::path::Path) {
    let output = child_command(mode, home, cache, receipts).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
    assert!(
        output.status.success(),
        "actual activation {mode}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Hold an exact synthetic child; cleanup never searches or signals unrelated processes.
struct NativeChild(std::process::Child);
impl NativeChild {
    /// Start a real native owner with file-backed diagnostics so a blocked child cannot fill a pipe.
    fn start(
        mode: &str,
        home: &std::path::Path,
        cache: &std::path::Path,
        receipts: &std::path::Path,
    ) -> Self {
        Self(
            child_command(mode, home, cache, receipts)
                .stdout(File::create(receipts.join("holder.stdout")).unwrap())
                .stderr(File::create(receipts.join("holder.stderr")).unwrap())
                .spawn()
                .unwrap(),
        )
    }
    /// Require a phase receipt from a still-live actual owner rather than treating exit as readiness.
    fn wait_ready(&mut self, receipts: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !receipts.join("ready").exists() {
            assert!(
                self.0.try_wait().unwrap().is_none(),
                "native holder exited before barrier: {} {}",
                fs::read_to_string(receipts.join("holder.stdout")).unwrap(),
                fs::read_to_string(receipts.join("holder.stderr")).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "native holder did not reach its actual phase barrier"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(self.0.try_wait().unwrap().is_none());
    }
    /// Kill exactly the retained native owner, collect it, then independently prove actual WRITER release.
    fn kill_and_prove_release(&mut self, receipts: &std::path::Path) {
        use std::os::unix::process::ExitStatusExt;
        self.0.kill().unwrap();
        let status = self.0.wait().unwrap();
        assert_eq!(
            status.signal(),
            Some(9),
            "exercise actual abrupt native termination"
        );
        let writer = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(fs::read_to_string(receipts.join("writer-path")).unwrap())
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&writer)
            .expect("PID/EOF alone is insufficient: actual WRITER must be independently available");
        drop(writer);
    }
}
impl Drop for NativeChild {
    /// Bound fixture cleanup to the exact spawned child even when a deciding assertion fails.
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Stop only this synthetic native owner at a known phase until the parent kills its exact Child.
fn barrier(receipts: &std::path::Path) -> ! {
    fs::write(
        receipts.join("writer-path"),
        namespace().join("WRITER").to_str().unwrap(),
    )
    .unwrap();
    fs::write(receipts.join("ready"), b"actual native owner reached phase").unwrap();
    loop {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Storage recovery and search generation contracts require real drop/restart and writer exclusion.
#[test]
fn actual_tuple_activation_recovery_and_writer_contention() {
    let Some(mode) = std::env::var_os("OMNI_ACTIVATION_MODE") else {
        for mode in [
            "post-switch-drop",
            "writer-cancel",
            "member-mutation",
            "pointer-cas",
            "kill-before-switch",
            "kill-after-switch",
            "retire-pinned",
            "retire-unknown",
            "retire-symlink",
            "retire-generation-race",
            "retire-readers-race",
            "model-change",
            "startup-watch",
        ] {
            let cache = tempfile::tempdir().unwrap();
            let home = tempfile::tempdir().unwrap();
            let receipts = tempfile::tempdir().unwrap();
            fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
            child("seed", home.path(), cache.path(), receipts.path());
            if mode == "writer-cancel" || mode.starts_with("kill-") {
                let holder_mode = if mode == "writer-cancel" {
                    "hold-native-writer"
                } else {
                    mode
                };
                let mut holder =
                    NativeChild::start(holder_mode, home.path(), cache.path(), receipts.path());
                holder.wait_ready(receipts.path());
                if mode == "writer-cancel" {
                    child(mode, home.path(), cache.path(), receipts.path());
                    assert!(
                        holder.0.try_wait().unwrap().is_none(),
                        "cancelled contender cannot terminate the real writer owner"
                    );
                }
                holder.kill_and_prove_release(receipts.path());
            } else {
                child(mode, home.path(), cache.path(), receipts.path());
            }
            child(
                if mode == "startup-watch" {
                    "read-dispatch"
                } else {
                    "read"
                },
                home.path(),
                cache.path(),
                receipts.path(),
            );
        }
        return;
    };
    let mode = mode.to_str().unwrap();
    let home = std::path::PathBuf::from(std::env::var_os("OMNI_ACTIVATION_HOME").unwrap());
    let receipts = std::path::PathBuf::from(std::env::var_os("OMNI_ACTIVATION_RECEIPTS").unwrap());
    let kb = if mode == "seed" {
        let kb = KnowledgeBase::init(&home, "Synthetic activation controls").unwrap();
        fs::write(home.join("raw/activation.md"), BODY).unwrap();
        kb
    } else {
        KnowledgeBase::open(&home).unwrap()
    };
    let (service, owner, _selection) = NativeLibraryService::ordinary_local(
        kb,
        LocalPolicy {
            mode: wire::ExposureMode::ReadWrite,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        },
    )
    .unwrap();
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    if mode == "read" || mode == "read-dispatch" {
        let expected = fs::read_to_string(receipts.join("selected")).unwrap();
        read_selected(&service, &owner, &session, &expected);
        if mode == "read-dispatch" {
            cold_query_selected(&service, &owner, &session);
        }
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0,
            "cold Query cannot fabricate Refresh settlement"
        );
        return;
    }
    if mode == "startup-watch" {
        let previous = current_stage();
        let permit = refresh_permit(&service, &session, mode, false);
        let inventory = reconcile(&service, &owner, permit.clone());
        let job = service
            .prepare_corpus_refresh(
                &owner,
                permit,
                inventory,
                None,
                holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0),
            )
            .unwrap();
        let need = job.corpus_requirement().unwrap();
        let submitted = job
            .submit(NativeJobPlan {
                operation_id: mode,
                job_id: mode,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 64 * 1024 * 1024,
                    max_millis: 120000,
                    max_memory_bytes: 512 * 1024 * 1024,
                },
                reservation: Reservation {
                    memory_bytes: 256 * 1024 * 1024,
                    temporary_bytes: need.temporary_bytes,
                },
                priority: knowledge_core::jobs::Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let account = Account::current().unwrap();
        let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
        assert_eq!(claim.job(), submitted.job);
        let (draft, report) = job
            .activate_corpus(claim, |_| {})
            .unwrap_or_else(|error| panic!("actual startup/watch activation: {:#}", error.cause()));
        assert_eq!(
            report.retired, 1,
            "one-call activation retires the actual unpinned prior tuple"
        );
        assert_eq!(report.pinned, 0);
        assert_eq!(report.retained_unknown, 0);
        let selected = current_stage();
        assert_ne!(selected, previous);
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "selection and retirement retain the actual Refresh until explicit settlement"
        );
        fs::write(receipts.join("selected"), selected).unwrap();
        complete(draft);
        return;
    }
    let previous = (mode != "seed").then(current_stage);
    let retained_source = if mode == "retire-pinned" {
        let (reader, permit) = open_reader(&service, &owner, &session);
        let source = bound_original(&service, &owner, &reader, &permit);
        drop(reader);
        drop(permit);
        Some(source)
    } else {
        None
    };
    let (model, model_fixture) = if mode == "model-change" {
        let (model, fixture) = corpus::contract_model(&service, &owner);
        (Some(model), Some(fixture))
    } else {
        (None, None)
    };
    let (mut draft, number, key) = prepared(&service, &owner, &session, mode, model);
    match mode {
        "seed" => {
            let draft = draft.select_tuple(|_| {}).unwrap();
            fs::write(receipts.join("selected"), current_stage()).unwrap();
            complete(draft);
        }
        "post-switch-drop" => {
            draft.test_switch_fault();
            let error = match draft.select_tuple(|_| {}) {
                Err(error) => error,
                Ok(_) => panic!("actual post-switch fault returns its owning draft"),
            };
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                1
            );
            let selected = current_stage();
            assert_ne!(
                Some(&selected),
                previous.as_ref(),
                "exercise a real pointer switch"
            );
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0,
                "actual error-owner drop collects and settles its exact Claim"
            );
            assert_eq!(
                current_stage(),
                selected,
                "foreground recovery preserves the actually switched tuple"
            );
            read_selected(&service, &owner, &session, &selected);
            fs::write(receipts.join("selected"), selected).unwrap();
        }
        "writer-cancel" => {
            let mut waits = 0;
            let error = match draft.select_tuple(|event| {
                if matches!(event, NativeCorpusProgress::WriterWaiting) {
                    waits += 1;
                    Account::current()
                        .unwrap()
                        .queue()
                        .cancel(number, key.key())
                        .unwrap();
                }
            }) {
                Err(error) => error,
                Ok(_) => panic!("a cancelled actual contending writer cannot publish"),
            };
            assert_eq!(
                waits, 1,
                "actual WRITER contention is visible before cancellation"
            );
            assert_eq!(current_stage(), *previous.as_ref().unwrap());
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                1,
                "cancel is not native collection or settlement"
            );
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0
            );
            let probe = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(namespace().join("WRITER"))
                .unwrap();
            assert_eq!(
                fs2::FileExt::try_lock_exclusive(&probe).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "dropping cancelled contender cannot unlock the other actual native writer"
            );
            drop(probe);
        }
        "hold-native-writer" => {
            let draft = draft.select_tuple(|_| {}).unwrap();
            fs::write(receipts.join("selected"), current_stage()).unwrap();
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                1,
                "kill barrier retains the actual selected draft and live Claim"
            );
            std::hint::black_box(&draft);
            barrier(&receipts);
        }
        "kill-before-switch" => {
            let _ = draft.select_tuple(|event| {
                if matches!(event, NativeCorpusProgress::WriteAdmitted { .. }) {
                    assert_eq!(current_stage(), *previous.as_ref().unwrap());
                    barrier(&receipts);
                }
            });
            panic!("fixture must be killed while its actual selector candidate is unswitched");
        }
        "kill-after-switch" => {
            let receipts = receipts.clone();
            draft.test_after_switch(move || {
                fs::write(receipts.join("selected"), current_stage()).unwrap();
                barrier(&receipts);
            });
            let _ = draft.select_tuple(|_| {});
            panic!("fixture must be killed after actual switch and before recovery effects");
        }
        "retire-generation-race" | "retire-readers-race" => {
            use std::os::unix::fs::MetadataExt;
            let prior = namespace().join(previous.as_ref().unwrap());
            let displaced = namespace().join("fixture-displaced-generation");
            let before = tree_signature(&prior);
            let readers_inode = fs::metadata(prior.join("READERS")).unwrap().ino();
            let mut draft = draft.select_tuple(|_| {}).unwrap();
            let selected = current_stage();
            let move_generation = mode == "retire-generation-race";
            let changed_prior = prior.clone();
            let changed_displaced = displaced.clone();
            draft.test_before_retirement_delete(move || {
                if move_generation {
                    fs::rename(&changed_prior, &changed_displaced).unwrap();
                    fs::create_dir(&changed_prior).unwrap();
                } else {
                    fs::rename(
                        changed_prior.join("READERS"),
                        changed_prior.join("fixture-displaced-readers"),
                    )
                    .unwrap();
                    fs::write(changed_prior.join("READERS"), b"").unwrap();
                }
            });
            let error = match draft.retire_previous(|_| {}) {
                Err(error) => error,
                Ok(_) => panic!(
                    "changed actual retirement generation/pin identity must return its owning refusal before deletion"
                ),
            };
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                1,
                "retirement refusal retains the actual native draft and Claim"
            );
            assert_eq!(current_stage(), selected);
            if move_generation {
                assert!(
                    prior.is_dir(),
                    "the competing replacement directory must survive"
                );
                assert_eq!(
                    fs::read_dir(&prior).unwrap().count(),
                    0,
                    "retirement cannot unlink the competing empty directory"
                );
                assert_eq!(
                    tree_signature(&displaced),
                    before,
                    "a generation moved after preflight must retain every original payload"
                );
            } else {
                assert_ne!(
                    fs::metadata(prior.join("READERS")).unwrap().ino(),
                    readers_inode,
                    "exercise equal-byte replacement of the actual aggregate pin inode"
                );
                assert!(prior.join("fixture-displaced-readers").is_file());
                fs::remove_file(prior.join("READERS")).unwrap();
                fs::rename(
                    prior.join("fixture-displaced-readers"),
                    prior.join("READERS"),
                )
                .unwrap();
                assert_eq!(
                    tree_signature(&prior),
                    before,
                    "pin replacement must be detected before any original payload/control deletion"
                );
            }
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0,
                "dropping the real refusal collects its exact native owner"
            );
            read_selected(&service, &owner, &session, &selected);
            fs::write(receipts.join("selected"), selected).unwrap();
        }
        "retire-pinned" | "retire-unknown" | "retire-symlink" => {
            let prior = namespace().join(previous.as_ref().unwrap());
            if mode == "retire-unknown" {
                fs::write(
                    prior.join("fixture-unknown"),
                    b"unowned cache conflict material",
                )
                .unwrap();
            } else if mode == "retire-symlink" {
                std::os::unix::fs::symlink(
                    home.join("raw/activation.md"),
                    prior.join("fixture-unknown"),
                )
                .unwrap();
            }
            let prior_signature = tree_signature(&prior);
            let draft = draft.select_tuple(|_| {}).unwrap();
            let selected = current_stage();
            assert_ne!(Some(&selected), previous.as_ref());
            let (draft, report) = draft.retire_previous(|_| {}).unwrap();
            assert_eq!(
                report.retired, 0,
                "pinned or unowned prior generations must survive intact"
            );
            assert!(prior.join("tuple.json").is_file());
            assert!(prior.join("representations.data").is_file());
            assert_eq!(
                tree_signature(&prior),
                prior_signature,
                "retirement refusal preserves every prior member and unknown entry"
            );
            if mode == "retire-pinned" {
                assert_eq!(
                    report.pinned, 1,
                    "the last actual source consumer keeps the whole tuple pinned"
                );
                assert_eq!(report.retained_unknown, 0);
                let source = retained_source.unwrap();
                source.check().unwrap();
                let mut bytes = vec![0; BODY.len()];
                source.file().unwrap().read_exact_at(&mut bytes, 0).unwrap();
                assert_eq!(bytes, BODY.as_bytes());
                source.check().unwrap();
                drop(source);
                let (draft, report) = draft.retire_previous(|_| {}).unwrap();
                assert_eq!(
                    report.retired, 1,
                    "retirement proceeds after the final real consumer releases its pin"
                );
                assert_eq!(report.pinned, 0);
                assert!(!prior.exists());
                complete(draft);
            } else {
                assert_eq!(
                    report.retained_unknown, 1,
                    "unknown files and symlinks preserve the whole previous generation"
                );
                assert_eq!(report.pinned, 0);
                assert!(fs::symlink_metadata(prior.join("fixture-unknown")).is_ok());
                complete(draft);
            }
            assert_eq!(
                fs::read_to_string(home.join("raw/activation.md")).unwrap(),
                BODY,
                "retirement never follows an unknown symlink into original data"
            );
            read_selected(&service, &owner, &session, &selected);
            fs::write(receipts.join("selected"), selected).unwrap();
        }
        "model-change" => {
            let (_replacement, _replacement_fixture) = corpus::contract_model(&service, &owner);
            let error = match draft.select_tuple(|_| {}) {
                Err(error) => error,
                Ok(_) => {
                    panic!("a sealed tuple cannot publish under a replaced actual model selection")
                }
            };
            assert_eq!(current_stage(), *previous.as_ref().unwrap());
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0
            );
            drop(model_fixture);
        }
        "member-mutation" => {
            let file = File::from(
                rustix::fs::openat(
                    draft.staging_directory().unwrap(),
                    "representations.data",
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .unwrap(),
            );
            file.write_all_at(b"!", 0).unwrap();
            file.sync_all().unwrap();
            let error = match draft.select_tuple(|_| {}) {
                Err(error) => error,
                Ok(_) => {
                    panic!("a mutated sealed member cannot replace the complete selected tuple")
                }
            };
            assert_eq!(current_stage(), *previous.as_ref().unwrap());
            drop(file);
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0
            );
        }
        "pointer-cas" => {
            let namespace = namespace();
            let old = fs::read(namespace.join("CURRENT")).unwrap();
            fs::write(namespace.join("fixture-competing-pointer"), &old).unwrap();
            fs::rename(
                namespace.join("fixture-competing-pointer"),
                namespace.join("CURRENT"),
            )
            .unwrap();
            let replaced = fs::metadata(namespace.join("CURRENT")).unwrap();
            let error = match draft.select_tuple(|_| {}) {
                Err(error) => error,
                Ok(_) => panic!("equal-byte competing pointer identity is a CAS conflict"),
            };
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(namespace.join("CURRENT")).unwrap().ino(),
                replaced.ino(),
                "the actual competing CURRENT must survive refusal"
            );
            assert_eq!(fs::read(namespace.join("CURRENT")).unwrap(), old);
            drop(error);
            assert_eq!(
                Account::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .queue
                    .unsettled,
                0
            );
        }
        _ => panic!("unknown synthetic activation mode"),
    }
}
