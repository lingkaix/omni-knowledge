//! Builder-owned real native writer/archive/reader composition; not independent acceptance or Mac proof.
use knowledge_core::admission::{Account, Configuration, Decision, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_package::{Capture, Input, Preset, Role, Window};
use knowledge_retrieval::{ArchiveBounds as Bounds, ArchiveQuotas as Quotas};
use knowledge_retrieval::{
    ArchiveSource, Content, Control, Document, Error, KernelAdapter, Locator, Mode, Query,
    ScopeView, SourceKey, StoredGeneration, StoredIdentity, StoredPlan, Work,
};
use omni_service::authority::{LocalPolicy, NativeAuthority, Permit, TransportFacts};
use omni_service::runtime::Runtime;
use omni_service::{Command, ExposureMode, Id, RelativePath, Transport};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

/// Build inputs bind actual source and consuming machinery; a digest is not read permission.
const BUILD: &str = concat!(env!("CARGO_PKG_NAME"), ":stored-generation-contract-v1");
/// The exact archived canonical source name is also the existing kernel writer's source name.
const SOURCE: &str = "indexes/native/source.txt";
/// Deterministic real source with distinct English, Chinese and Japanese lexical/scan evidence.
const TEXT: &[u8] = "alpha shared archive\n机器学习检索\n日本語の検索エンジン\n".as_bytes();
/// Hash controlled fixture bytes, not an authority label.
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Use the actual process account, never a numeric stand-in grant.
fn reserve(need: Reservation) -> Work {
    match Work::reserve(need, Origin::BeforeVerification).unwrap() {
        Decision::Ready(work) => work,
        Decision::Waiting(reason) => panic!("fixture unexpectedly waiting: {reason:?}"),
    }
}
/// Keep original coordinates and source identity across filesystem and archived generations.
fn view() -> ScopeView {
    ScopeView {
        scope: "archive-shard".into(),
        unavailable_objects: 0,
        documents: vec![Document {
            key: SourceKey {
                namespace: "archive-contract".into(),
                id: "first-source".into(),
            },
            revision: digest(TEXT),
            digest: digest(TEXT),
            size: TEXT.len() as u64,
            name: "source.txt".into(),
            title: "Canonical archive".into(),
            media_type: "text/plain".into(),
            content: Content::Text {
                path: SOURCE.into(),
                range: None,
            },
            text_ranges: None,
            language: Some("unicode".into()),
            metadata: BTreeMap::new(),
            locator: Some("pages/source".into()),
        }],
    }
}
/// Full finite M5 verification ceilings, not an archive-size or corpus cap increase.
fn bounds() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 100_000,
            max_bytes: 128 * 1024 * 1024,
            max_millis: 60_000,
            max_memory_bytes: 1024 * 1024 * 1024,
        },
        quotas: Quotas {
            max_archive_bytes: 32 * 1024 * 1024,
            max_entries: 512,
            max_entry_expanded: 8 * 1024 * 1024,
            max_total_expanded: 32 * 1024 * 1024,
            max_ratio: 1000,
            max_path_bytes: 1024,
            max_depth: 16,
        },
    }
}
/// Gather only actual writer-created native components; this is bounded fixture setup, not ingestion.
fn components(directory: &Path, relative: &Path, output: &mut Vec<(String, File)>) {
    for entry in fs::read_dir(directory.join(relative)).unwrap() {
        let entry = entry.unwrap();
        let child = relative.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            components(directory, &child, output);
        } else {
            output.push((
                format!("indexes/native/{}", child.to_str().unwrap()),
                File::open(entry.path()).unwrap(),
            ));
        }
    }
}
/// Public M5 capture produces the inventory; the external package producer adds its actual native tuple.
/// M5 capture itself still omits indexes. Native optional identity is external wire data, never a grant.
fn archive_fixture(
    root: &Path,
    native_dir: &Path,
    source: &Path,
    generation: &str,
    account: &Account,
) -> PathBuf {
    let mut files = Vec::new();
    components(native_dir, Path::new(""), &mut files);
    files.push((SOURCE.into(), File::open(source).unwrap()));
    let paths: Vec<_> = files.iter().map(|(name, _)| name.clone()).collect();
    let output = root.join("captured.zip");
    let private = root.join("capture-private");
    fs::create_dir(&private).unwrap();
    let limits = bounds();
    let cancel = AtomicBool::new(false);
    let mut held = match account
        .reserve(
            Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        Decision::Waiting(r) => panic!("fixture waiting {r:?}"),
    };
    let mut job = knowledge_package::Work::new(&limits, &mut held, &cancel).unwrap();
    let mut inputs: Vec<_> = files
        .iter_mut()
        .map(|(name, file)| Input {
            path: knowledge_core::data::RelativePath::new(name.as_str()).unwrap(),
            role: Role::Index,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: file,
        })
        .collect();
    let mut request = Capture {
        operation_id: "native-archive",
        preset: Preset::Frozen,
        library_id: "archive-library",
        name: "Native archive",
        text_root: "pages",
        window: Window {
            start: "2026-11-05T11:00:00Z".into(),
            end: "2026-11-05T11:00:01Z".into(),
        },
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: true,
        required_features: vec![],
        private_dir: &private,
        destination: &output,
    };
    let captured = knowledge_package::capture(&mut request, &mut inputs, &mut job).unwrap();
    drop(captured);
    drop(inputs);
    drop(files);
    drop(job);
    held.finish().unwrap();
    let native = StoredIdentity::new(BUILD).unwrap();
    let requirements = native.requirements(generation);
    let mut zip = zip::ZipArchive::new(File::open(output).unwrap()).unwrap();
    let final_path = root.join("frozen.zip");
    let mut writer = zip::ZipWriter::new(File::create(&final_path).unwrap());
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if member.name() == "manifest.json" {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            value["indexes"] = serde_json::json!({"compatible": true, "paths": paths, "identity": {
                "version": 1, "kernel_build": requirements.kernel_build, "lexical_format": requirements.lexical_format,
                "segment_format": requirements.segment_format, "processor": requirements.processor, "dictionary": requirements.dictionary,
                "generation": generation, "model_space": null }});
            value["coverage"]["indexes"] = serde_json::json!("complete");
            bytes = serde_json::to_vec(&value).unwrap();
        }
        writer
            .start_file(
                member.name(),
                zip::write::SimpleFileOptions::default().compression_method(member.compression()),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().sync_all().unwrap();
    final_path
}
/// Actual service owner-selected FD and current policy/version carrier used by the archive adapter.
struct SelectedArchive {
    file: File,
    permit: Permit,
    library: Id,
    path: RelativePath,
}
impl ArchiveSource for SelectedArchive {
    /// Check the real service permit and descriptor-relative current source, not a selected constant.
    fn check(&self) -> Result<(), Error> {
        self.permit
            .check_path(&self.library, &self.path)
            .map_err(|_| Error::Inventory)?;
        self.permit
            .with_root(&self.library, |root| {
                let fd = rustix::fs::openat(
                    root,
                    self.path.as_str(),
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| omni_service::ErrorCode::Changed)?;
                let now = File::from(fd)
                    .metadata()
                    .map_err(|_| omni_service::ErrorCode::Changed)?;
                let held = self
                    .file
                    .metadata()
                    .map_err(|_| omni_service::ErrorCode::Changed)?;
                if (
                    now.dev(),
                    now.ino(),
                    now.len(),
                    now.mtime(),
                    now.mtime_nsec(),
                    now.ctime(),
                    now.ctime_nsec(),
                ) != (
                    held.dev(),
                    held.ino(),
                    held.len(),
                    held.mtime(),
                    held.mtime_nsec(),
                    held.ctime(),
                    held.ctime_nsec(),
                ) {
                    return Err(omni_service::ErrorCode::Changed);
                }
                Ok(())
            })
            .map_err(|_| Error::Inventory)
    }
    /// Return only the original held native descriptor after the current owner check.
    fn file(&self) -> Result<&File, Error> {
        self.check()?;
        Ok(&self.file)
    }
}
/// Bind the actual running Claim and its complete observed key to every controlled native read.
fn claim_control(
    source: Arc<SelectedArchive>,
    claim: Arc<knowledge_core::jobs::Claim>,
    key: Key<'_>,
) -> Control {
    let context = key.context.to_owned();
    let library = key.library.to_owned();
    let mount = key.mount.map(str::to_owned);
    let source_id = key.source.to_owned();
    let version = key.version.clone();
    let recipe = key.recipe.clone();
    let processor = key.processor.clone();
    Control::guarded(
        claim.deadline(),
        Arc::new(move || {
            source.check().is_ok()
                && claim
                    .stop_requested(Key {
                        context: &context,
                        library: &library,
                        mount: mount.as_deref(),
                        source: &source_id,
                        version: &version,
                        recipe: &recipe,
                        processor: &processor,
                    })
                    .is_ok_and(|stop| stop.is_none())
        }),
    )
}

/// Search through the real archive-backed Directory and mature shared query implementation.
fn query(
    generation: StoredGeneration,
    mode: Mode,
    text: &str,
    control: &Control,
) -> knowledge_retrieval::StoredResult {
    generation
        .search(
            Query {
                mode,
                query: text.into(),
                scopes: vec![view()],
                limit: 10,
            },
            control,
        )
        .unwrap()
}
/// Compose actual writer/M5 ranges/Tantivy queries, native policy and Claim/resource lifetimes.
#[test]
fn actual_stored_generation_queries_and_current_native_lifetimes() {
    let cache = tempfile::tempdir().unwrap();
    fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let account = Account::start(
        File::open(cache.path()).unwrap(),
        Configuration {
            memory_bytes: Some(2 * 1024 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 16,
            max_records: 128,
            max_descriptor_bytes: 1024 * 1024,
            per_mount: 2,
        },
    )
    .unwrap();
    let source_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(source_root.path().join("indexes/native")).unwrap();
    let source_path = source_root.path().join(SOURCE);
    fs::write(&source_path, TEXT).unwrap();
    let native_store = cache.path().join("kernel");
    let kernel = KernelAdapter::open(source_root.path(), &native_store, BUILD).unwrap();
    let control = Control::new(Instant::now() + Duration::from_secs(60));
    let mut publication = reserve(KernelAdapter::publish_requirement(4096).unwrap());
    let generation_id = kernel
        .publish(view(), None, &mut publication, &control)
        .unwrap();
    drop(publication);
    let scope_dir = fs::read_dir(&native_store)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let selected_parent = tempfile::tempdir().unwrap();
    let selected_root = selected_parent.path().join("selected");
    fs::create_dir(&selected_root).unwrap();
    let archive = archive_fixture(
        &selected_root,
        &scope_dir.join(&generation_id),
        &source_path,
        &generation_id,
        account,
    );
    drop(kernel);
    fs::remove_dir_all(&native_store).unwrap();
    fs::remove_file(&source_path).unwrap();
    // No unpack/scratch/rebuild is possible: both the native store and canonical loose source are gone.
    let (authority, owner) = NativeAuthority::new();
    let owner = Arc::new(owner);
    let selected_path = selected_root.clone();
    let policy = LocalPolicy {
        mode: ExposureMode::ReadOnly,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let library = Id::new("archive-library").unwrap();
    let selection = owner
        .select_current(
            library.clone(),
            File::open(&selected_root).unwrap(),
            policy.clone(),
            Arc::new(move |root| {
                let held = root.metadata().unwrap();
                fs::metadata(&selected_path)
                    .is_ok_and(|now| (now.dev(), now.ino()) == (held.dev(), held.ino()))
            }),
        )
        .unwrap();
    let session = authority
        .connect(std::slice::from_ref(&selection), TransportFacts::OwnerStdio)
        .unwrap();
    let runtime = Runtime::new(authority);
    let command: Command = serde_json::from_value(serde_json::json!({"op":"browse", "library_id":library.as_str(), "mount_id":null, "path":"", "cursor":null, "limit":10})).unwrap();
    let permit = runtime.native_scope(&session, &command, None).unwrap();
    let file = permit
        .with_root(&library, |root| {
            rustix::fs::openat(
                root,
                "frozen.zip",
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(|_| omni_service::ErrorCode::Changed)
        })
        .unwrap();
    let context = permit
        .with_selected_context(&library, |root, policy, revision| {
            let m = root.metadata().unwrap();
            Ok(format!("{}:{}:{policy:?}:{revision}", m.dev(), m.ino()))
        })
        .unwrap();
    let source = Arc::new(SelectedArchive {
        file,
        permit,
        library: library.clone(),
        path: RelativePath::new("frozen.zip").unwrap(),
    });
    let version = ContentDigest::new(digest(&fs::read(&archive).unwrap())).unwrap();
    let recipe = ContentDigest::new(digest(b"real native archive query")).unwrap();
    let processor = ContentDigest::new(digest(BUILD.as_bytes())).unwrap();
    let key = Key {
        context: &context,
        library: library.as_str(),
        mount: None,
        source: "frozen.zip",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    let plan = StoredPlan {
        index_bytes: 2 * 1024 * 1024,
        table_bytes: 2 * 1024 * 1024,
    };
    let need = plan.requirement().unwrap();
    account
        .submit(Submission {
            operation_id: "archive-query",
            job_id: "archive-query-job",
            key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let work = Work::split_claim(&claim, key, need).unwrap();
    let identity = StoredIdentity::new(BUILD).unwrap();
    let requirements = identity.requirements(&generation_id);
    let guarded = claim_control(source.clone(), claim.clone(), key);
    // Spend all otherwise free global credit after the complete real query plan
    // was paid. Verification must borrow that plan, not make a second late charge.
    let pressure = reserve(Reservation {
        memory_bytes: account.status().unwrap().memory_available,
        temporary_bytes: 0,
    });
    let mut reader = StoredGeneration::open(
        source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        work,
        &bounds(),
        &AtomicBool::new(false),
        &guarded,
    )
    .unwrap();
    assert_eq!(reader.id(), generation_id);
    drop(pressure);
    let baseline = account.status().unwrap().reserved.memory_bytes;
    for (mode, text) in [
        (Mode::Lexical, "alpha"),
        (Mode::Lexical, "机器学习"),
        (Mode::Lexical, "日本語"),
        (Mode::Exact, "shared archive"),
        (Mode::Regex, "^alpha.*archive$"),
    ] {
        let result = query(reader, mode, text, &control);
        assert!(
            result.result().complete,
            "{mode:?}: {:?}",
            result.result().coverage
        );
        assert_eq!(result.result().hits.len(), 1);
        assert_eq!(result.result().hits[0].key.id, "first-source");
        assert_eq!(result.result().hits[0].digest, digest(TEXT));
        assert!(matches!(
            &result.result().hits[0].locator,
            Locator::Text { original: Some(original), .. } if original == "pages/source"
        ));
        if mode == Mode::Exact {
            assert_eq!(
                result.result().hits[0].locator,
                Locator::Text {
                    start: 6,
                    end: 20,
                    original: Some("pages/source".into())
                }
            );
        }
        if mode == Mode::Regex {
            assert_eq!(
                result.result().hits[0].locator,
                Locator::Text {
                    start: 0,
                    end: 20,
                    original: Some("pages/source".into())
                }
            );
        }
        assert_eq!(result.result().generations["archive-shard"], generation_id);
        result.check().unwrap();
        reader = result.into_generation();
    }
    let cancellation_recipe =
        ContentDigest::new(digest(b"real cancellable native archive query")).unwrap();
    let cancellation_key = Key {
        recipe: &cancellation_recipe,
        ..key
    };
    account
        .submit(Submission {
            operation_id: "archive-stop",
            job_id: "archive-stop-job",
            key: cancellation_key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let stopping_claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let stopping_work = Work::split_claim(&stopping_claim, cancellation_key, need).unwrap();
    let stopping_control = claim_control(source.clone(), stopping_claim.clone(), cancellation_key);
    let stopping_reader = StoredGeneration::open(
        source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        stopping_work,
        &bounds(),
        &AtomicBool::new(false),
        &stopping_control,
    )
    .unwrap();
    let stopping_result = query(stopping_reader, Mode::Lexical, "alpha", &stopping_control);
    account
        .queue()
        .cancel(stopping_claim.job(), cancellation_key)
        .unwrap();
    source.check().unwrap();
    assert!(
        stopping_result.check().is_err(),
        "actual Claim cancellation, not a changed source or policy, blocks delivery"
    );
    let stopping_reader = stopping_result.into_generation();
    assert!(
        stopping_reader
            .search(
                Query {
                    mode: Mode::Lexical,
                    query: "alpha".into(),
                    scopes: vec![view()],
                    limit: 10
                },
                &control
            )
            .is_err()
    );
    drop(stopping_control);
    account
        .queue()
        .settle(&stopping_claim, cancellation_key, Outcome::Cancelled)
        .unwrap();
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        baseline,
        "stopped native reader costs settle only after their real owners drop"
    );
    let delivered = query(reader, Mode::Lexical, "alpha", &control);
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        baseline,
        "result owner retains the actual native reader charge"
    );
    let renamed = selected_parent.path().join("old-selected");
    fs::rename(&selected_root, &renamed).unwrap();
    fs::create_dir(&selected_root).unwrap();
    assert!(
        delivered.check().is_err(),
        "held FDs do not authorize a replaced selected parent"
    );
    fs::remove_dir(&selected_root).unwrap();
    fs::rename(&renamed, &selected_root).unwrap();
    delivered.check().unwrap();

    // Revoke a real native selection DURING initial M5 verification, not just
    // before open or after query. A later bad CRC makes an unguarded full pass
    // observably different: it fails checksum without ever reaching revocation.
    let bad_path = selected_root.join("bad.zip");
    fs::copy(&archive, &bad_path).unwrap();
    let offset = {
        let mut zip = zip::ZipArchive::new(File::open(&bad_path).unwrap()).unwrap();
        zip.by_name(SOURCE).unwrap().data_start()
    };
    {
        use std::os::unix::fs::FileExt;
        let file = fs::OpenOptions::new().write(true).open(&bad_path).unwrap();
        file.write_all_at(b"!", offset).unwrap();
        file.sync_all().unwrap();
    }
    let permit = runtime.native_scope(&session, &command, None).unwrap();
    let bad_source = Arc::new(SelectedArchive {
        file: File::open(&bad_path).unwrap(),
        permit,
        library: library.clone(),
        path: RelativePath::new("bad.zip").unwrap(),
    });
    let bad_version = ContentDigest::new(digest(&fs::read(&bad_path).unwrap())).unwrap();
    let bad_key = Key {
        source: "bad.zip",
        version: &bad_version,
        ..key
    };
    account
        .submit(Submission {
            operation_id: "archive-refill-revoke",
            job_id: "archive-refill-revoke-job",
            key: bad_key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let bad_claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let bad_work = Work::split_claim(&bad_claim, bad_key, need).unwrap();
    let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let check_count = checks.clone();
    let native_owner = owner.clone();
    let native_selection = selection.clone();
    let mut revoked_policy = policy.clone();
    revoked_policy.excluded_paths = vec![RelativePath::new("bad.zip").unwrap()];
    let bad_current = claim_control(bad_source.clone(), bad_claim.clone(), bad_key);
    let revoke = Control::guarded(
        bad_claim.deadline(),
        Arc::new(move || {
            let n = check_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if n == 16 {
                native_owner
                    .set_policy(&native_selection, revoked_policy.clone())
                    .unwrap();
            }
            bad_current.check().is_ok()
        }),
    );
    let opening = StoredGeneration::open(
        bad_source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        bad_work,
        &bounds(),
        &AtomicBool::new(false),
        &revoke,
    );
    assert!(
        matches!(
            opening,
            Err(Error::Package(knowledge_package::Error::Cancelled))
        ),
        "initial verification returned {:?}",
        opening.as_ref().err(),
    );
    assert_eq!(checks.load(std::sync::atomic::Ordering::SeqCst), 16);
    assert!(
        bad_source.check().is_err(),
        "actual owner revocation took effect inside verification"
    );
    account.queue().cancel(bad_claim.job(), bad_key).unwrap();
    account
        .queue()
        .settle(&bad_claim, bad_key, Outcome::Cancelled)
        .unwrap();
    drop(revoke);
    drop(bad_source);
    drop(bad_claim);
    owner
        .set_policy(
            &selection,
            LocalPolicy {
                excluded_paths: vec![RelativePath::new("frozen.zip").unwrap()],
                ..policy
            },
        )
        .unwrap();
    assert!(
        delivered.check().is_err(),
        "current native policy revocation withholds old result bytes"
    );
    drop(delivered);
    drop(source);
    drop(guarded);
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    assert!(account.status().unwrap().reserved.memory_bytes < baseline - need.memory_bytes / 2);
    assert!(matches!(
        Work::reserve(
            Reservation {
                memory_bytes: 2 * 1024 * 1024 * 1024 + 1,
                temporary_bytes: 0
            },
            Origin::BeforeVerification
        ),
        Err(Error::Admission(
            knowledge_core::admission::Error::TooLargeMemory
        ))
    ));
    let used = account.status().unwrap().reserved.memory_bytes;
    assert!(matches!(
        Work::reserve(
            Reservation {
                memory_bytes: 2 * 1024 * 1024 * 1024 - used + 1,
                temporary_bytes: 0
            },
            Origin::BeforeVerification
        )
        .unwrap(),
        Decision::Waiting(_)
    ));
}
