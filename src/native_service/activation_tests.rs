//! Real signed native ZIP -> selected native trust owner -> actual STORED kernel query.
//! These controls use synthetic keys and roots; no imported trust callback or loose-store fallback.
use super::*;
use crate::native_service::embedding_tests::{digest, ready};
use knowledge_core::admission::{Account, Decision, Holding, Origin};
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_history::{
    History, KnownSigner, Mode as HistoryMode, NativeHistoryOwner, OwnerContext, SignatureState,
};
use knowledge_package::{Bounds, Capture, Input, Preset, Quotas, Role, Window};
use knowledge_retrieval::{
    ArchiveSource, Content, Control, Document, KernelAdapter, Mode, Query, ScopeView, SourceKey,
    StoredIdentity, StoredPlan, Work,
};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, atomic::AtomicBool};

const TEST_NAME: &str =
    "native_service::activation_tests::actual_signed_frozen_archive_native_trust_and_stored_query";
const SOURCE: &str = "indexes/native/source.txt";
const TEXT: &[u8] = "alpha shared archive\n机器学习检索\n日本語の検索エンジン\n".as_bytes();

/// Finite existing package ceilings; metadata memory is a separate same-Claim verifier loan.
fn bounds() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 100_000,
            max_bytes: 32 * 1024 * 1024,
            max_millis: 60000,
            max_memory_bytes: 64 * 1024 * 1024,
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

/// Actual whole-job ceilings retain the native verifier and reader's complete quoted peaks.
fn job_budget() -> Budget {
    Budget {
        max_memory_bytes: 512 * 1024 * 1024,
        ..bounds().budget
    }
}

/// The canonical document's original path/coordinates survive packaging and real kernel querying.
fn view() -> ScopeView {
    ScopeView {
        scope: "snapshot-shard".into(),
        unavailable_objects: 0,
        documents: vec![Document {
            key: SourceKey {
                namespace: "snapshot-contract".into(),
                id: "snapshot-source".into(),
            },
            revision: hex::encode(Sha256::digest(TEXT)),
            digest: hex::encode(Sha256::digest(TEXT)),
            size: TEXT.len() as u64,
            name: "source.txt".into(),
            title: "Selected native snapshot".into(),
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

/// Admit actual fixture work on the one process account, never a numerical grant.
fn holding(need: Reservation) -> Holding {
    match Account::current()
        .unwrap()
        .reserve(need, Origin::BeforeVerification)
        .unwrap()
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(reason) => panic!("actual snapshot fixture admission: {reason:?}"),
    }
}

/// Collect only files emitted by the real kernel writer into the synthetic archive producer.
fn components(root: &Path, relative: &Path, out: &mut Vec<(String, File)>) {
    for entry in fs::read_dir(root.join(relative)).unwrap() {
        let entry = entry.unwrap();
        let path = relative.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            components(root, &path, out);
        } else {
            out.push((
                format!("indexes/native/{}", path.to_str().unwrap()),
                File::open(entry.path()).unwrap(),
            ));
        }
    }
}

/// Run trusted system SSH tools only against task-owned disposable key and payload paths.
fn ssh(args: &[&std::ffi::OsStr]) {
    let output = Command::new("/usr/bin/ssh-keygen")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "synthetic SSHSIG tool failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Preserve producer metadata and payload bytes while appending only an external detached signature.
fn write_zip(path: &Path, entries: &[(String, Vec<u8>, zip::CompressionMethod)]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, bytes, method) in entries {
        writer
            .start_file(
                name.as_str(),
                zip::write::SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().sync_all().unwrap();
}

/// Public producer controls retain omission, valid zero-byte members, and invalid-selection refusal.
fn producer_controls(root: &Path, native: &knowledge_package::IndexRequirements<'_>) {
    for mode in ["ordinary", "bad-identity", "zero-member", "no-index"] {
        let private = root.join(format!("producer-{mode}"));
        fs::create_dir(&private).unwrap();
        let destination = root.join(format!("producer-{mode}.zip"));
        let limits = bounds();
        let cancel = AtomicBool::new(false);
        let mut credit = holding(Reservation {
            memory_bytes: 0,
            temporary_bytes: 0,
        });
        {
            let mut work = knowledge_package::Work::new(&limits, &mut credit, &cancel).unwrap();
            let mut payload = std::io::Cursor::new(if mode == "zero-member" {
                &b""[..]
            } else {
                &b"actual selected bytes"[..]
            });
            let mut inputs = [Input {
                path: knowledge_core::data::RelativePath::new("indexes/native/component").unwrap(),
                role: if mode == "no-index" {
                    Role::Core
                } else {
                    Role::Index
                },
                mount_id: None,
                source_id: None,
                expected: None,
                reader: &mut payload,
            }];
            let mut capture = Capture {
                operation_id: "producer-control",
                preset: Preset::Frozen,
                library_id: "synthetic-producer",
                name: "Native producer controls",
                text_root: "pages",
                window: Window {
                    start: "2026-11-09T00:00:00Z".into(),
                    end: "2026-11-09T00:00:01Z".into(),
                },
                mounts: vec![],
                external: vec![],
                omitted: 0,
                require_complete: true,
                required_features: vec![],
                private_dir: &private,
                destination: &destination,
            };
            if mode == "ordinary" || mode == "zero-member" {
                let captured = if mode == "ordinary" {
                    knowledge_package::capture(&mut capture, &mut inputs, &mut work)
                } else {
                    knowledge_package::capture_with_indexes(
                        &mut capture,
                        &mut inputs,
                        native,
                        &mut work,
                    )
                }
                .unwrap();
                assert!(captured.linked);
                let mut zip = zip::ZipArchive::new(File::open(&destination).unwrap()).unwrap();
                let mut bytes = Vec::new();
                zip.by_name("manifest.json")
                    .unwrap()
                    .read_to_end(&mut bytes)
                    .unwrap();
                let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                if mode == "ordinary" {
                    assert_eq!(manifest["indexes"]["compatible"], false);
                    assert_eq!(manifest["indexes"]["reason"], "kernel_not_wired");
                    assert_eq!(manifest["coverage"]["indexes"], "omitted");
                } else {
                    assert_eq!(manifest["indexes"]["compatible"], true);
                    assert_eq!(manifest["coverage"]["indexes"], "complete");
                    assert_eq!(manifest["entries"][0]["size_bytes"], "0");
                    assert_eq!(manifest["entries"][0]["sha256"], digest(b"").as_str());
                }
                assert_eq!(captured.snapshot_id, digest(&bytes));
            } else {
                let selected = if mode == "bad-identity" {
                    knowledge_package::IndexRequirements {
                        generation: "",
                        ..*native
                    }
                } else {
                    *native
                };
                assert_eq!(
                    knowledge_package::capture_with_indexes(
                        &mut capture,
                        &mut inputs,
                        &selected,
                        &mut work
                    )
                    .unwrap_err(),
                    knowledge_package::Error::Invalid,
                    "{mode} must not promote invalid identity or absent index selection to complete native indexes"
                );
                assert!(!destination.exists());
                assert_eq!(
                    fs::read_dir(&private).unwrap().count(),
                    0,
                    "invalid native identity/selection refuses before staging"
                );
            }
        }
        credit.finish().unwrap();
    }
}

/// Real kernel + public qualified M5 capture create the immutable manifest before external signing.
fn package(root: &Path, output: &Path, mode: &str) -> (String, ContentDigest, String) {
    let source_root = root.join("loose-source");
    fs::create_dir_all(source_root.join("indexes/native")).unwrap();
    let source = source_root.join(SOURCE);
    fs::write(&source, TEXT).unwrap();
    let store = root.join("loose-kernel");
    let build = NativeTrustedArchive::build_identity();
    let kernel = KernelAdapter::open(&source_root, &store, &build).unwrap();
    let control = Control::new(Instant::now() + Duration::from_secs(60));
    let mut work = ready(KernelAdapter::publish_requirement(4096).unwrap());
    let generation = kernel.publish(view(), None, &mut work, &control).unwrap();
    drop(work);
    let scope = fs::read_dir(&store)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut files = Vec::new();
    components(&scope.join(&generation), Path::new(""), &mut files);
    files.push((SOURCE.into(), File::open(&source).unwrap()));
    let paths: Vec<_> = files.iter().map(|(path, _)| path.clone()).collect();
    let identity = StoredIdentity::new(&build).unwrap();
    let requirements = identity.requirements(&generation);
    if mode == "producer-controls" {
        producer_controls(root, &requirements);
    }
    let private = root.join("capture-private");
    fs::create_dir(&private).unwrap();
    let captured_path = root.join("captured.zip");
    let limits = bounds();
    let cancel = AtomicBool::new(false);
    let mut credit = holding(Reservation {
        memory_bytes: 0,
        temporary_bytes: 0,
    });
    let captured_snapshot;
    {
        let mut package_work = knowledge_package::Work::new(&limits, &mut credit, &cancel).unwrap();
        let mut inputs: Vec<_> = files
            .iter_mut()
            .map(|(path, file)| Input {
                path: knowledge_core::data::RelativePath::new(path.as_str()).unwrap(),
                role: Role::Index,
                mount_id: None,
                source_id: None,
                expected: None,
                reader: file,
            })
            .collect();
        let mut capture = Capture {
            operation_id: "actual-native-snapshot",
            preset: Preset::Frozen,
            library_id: "packaged-library",
            name: "Synthetic native signed snapshot",
            text_root: "pages",
            window: Window {
                start: "2026-11-09T00:00:00Z".into(),
                end: "2026-11-09T00:00:01Z".into(),
            },
            mounts: vec![],
            external: vec![],
            omitted: 0,
            require_complete: true,
            required_features: vec![],
            private_dir: &private,
            destination: &captured_path,
        };
        let captured = knowledge_package::capture_with_indexes(
            &mut capture,
            &mut inputs,
            &requirements,
            &mut package_work,
        )
        .unwrap();
        assert!(captured.linked);
        captured_snapshot = captured.snapshot_id;
    }
    credit.finish().unwrap();
    drop(files);
    let mut zip = zip::ZipArchive::new(File::open(&captured_path).unwrap()).unwrap();
    let mut entries = Vec::new();
    let mut manifest = Vec::new();
    for index in 0..zip.len() {
        let mut member = zip.by_index(index).unwrap();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if member.name().unwrap() == "manifest.json" {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let mut ordered = paths.clone();
            ordered.sort();
            assert_eq!(
                value["indexes"],
                serde_json::json!({"compatible":true,"paths":ordered,"identity":{
                "version":1,"kernel_build":requirements.kernel_build,"lexical_format":requirements.lexical_format,
                "segment_format":requirements.segment_format,"processor":requirements.processor,"dictionary":requirements.dictionary,
                "generation":generation,"model_space":null }})
            );
            assert_eq!(value["coverage"]["indexes"], "complete");
            // This controlled schema has sorted ASCII keys and integer/string values, the RFC8785 subset.
            assert_eq!(
                bytes,
                serde_json::to_vec(&value).unwrap(),
                "producer emits canonical manifest bytes without fixture metadata rewriting"
            );
            manifest = bytes.clone();
        }
        entries.push((
            member.name().unwrap().into_owned(),
            bytes,
            member.compression(),
        ));
    }
    assert!(!manifest.is_empty());
    let snapshot = digest(&manifest);
    assert_eq!(
        captured_snapshot, snapshot,
        "producer snapshot is the independently hashed original canonical manifest"
    );
    let keys = root.join("disposable-keys");
    fs::create_dir(&keys).unwrap();
    fs::set_permissions(&keys, fs::Permissions::from_mode(0o700)).unwrap();
    let key = keys.join("signing-key");
    ssh(&[
        "-q".as_ref(),
        "-t".as_ref(),
        "ed25519".as_ref(),
        "-N".as_ref(),
        "".as_ref(),
        "-f".as_ref(),
        key.as_os_str(),
    ]);
    let public = fs::read_to_string(key.with_extension("pub")).unwrap();
    if !matches!(mode, "unsigned-required" | "unsigned-explicit") {
        let decoded = hex::decode(snapshot.as_str()).unwrap();
        assert_eq!(decoded.len(), 32);
        let payload = keys.join("snapshot-digest");
        fs::write(
            &payload,
            if matches!(mode, "invalid" | "invalid-explicit") {
                vec![0; 32]
            } else {
                decoded
            },
        )
        .unwrap();
        if matches!(mode, "invalid" | "invalid-explicit") {
            assert_ne!(snapshot.as_str(), "0".repeat(64));
        }
        let namespace = if mode == "wrong-namespace" {
            "git"
        } else {
            "omni-knowledge.snapshot.v1"
        };
        ssh(&[
            "-Y".as_ref(),
            "sign".as_ref(),
            "-n".as_ref(),
            namespace.as_ref(),
            "-f".as_ref(),
            key.as_os_str(),
            payload.as_os_str(),
        ]);
        let signature = fs::read(payload.with_extension("sig")).unwrap();
        entries.push((
            "manifest.sig".into(),
            signature,
            zip::CompressionMethod::Stored,
        ));
    }
    write_zip(output, &entries);
    drop(zip);
    drop(kernel);
    fs::remove_dir_all(&store).unwrap();
    fs::remove_dir_all(&source_root).unwrap();
    fs::remove_file(&captured_path).unwrap();
    fs::remove_dir_all(private).unwrap();
    assert!(
        !store.exists() && !source.exists(),
        "only the native ZIP can supply this generation's bytes"
    );
    (generation, snapshot, public)
}

/// Keep the full actual queue key while the native processing carrier moves into its archive owner.
struct OwnedKey {
    context: String,
    library: String,
    source: String,
    version: ContentDigest,
    recipe: ContentDigest,
    processor: ContentDigest,
}
impl OwnedKey {
    /// Copy only identity data; no grant, Claim or matching deadline is recreated.
    fn new(key: Key<'_>) -> Self {
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
    /// Borrow the exact original full key for actual cancellation/settlement.
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

/// Initialize the existing real History host before archive verification, then settle its native setup.
fn history(root: &Path, home: &Path) -> History {
    let version = digest(b"native history setup");
    let key = Key {
        context: "fixture-history-setup",
        library: "setup",
        mount: None,
        source: "history",
        version: &version,
        recipe: &version,
        processor: &version,
    };
    let limits = Budget {
        max_files: 1000,
        max_bytes: 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 64 * 1024 * 1024,
    };
    let account = Account::current().unwrap();
    account
        .submit(Submission {
            key,
            operation_id: "history-setup",
            job_id: "history-setup",
            budget: limits,
            reservation: History::required_resources(&limits).unwrap(),
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    let history = History::init(
        &root.join("native-history.git"),
        home,
        "wiki",
        OwnerContext {
            name: "Synthetic Snapshot Owner".into(),
            email: "snapshot@example.invalid".into(),
            agent: None,
            tool: None,
            session: None,
            mode: HistoryMode::UnsignedExplicit,
        },
        &claim,
    )
    .unwrap();
    assert!(history.pending_process(&claim).unwrap().is_none());
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    history
}

/// Actual source/Claim/native signer policy and real STORED query are composed without fake owners.
#[test]
fn actual_signed_frozen_archive_native_trust_and_stored_query() {
    let Some(mode) = std::env::var_os("OMNI_SNAPSHOT_MODE") else {
        for mode in [
            "known",
            "unknown",
            "unknown-explicit",
            "invalid",
            "invalid-explicit",
            "wrong-namespace",
            "unsigned-required",
            "unsigned-explicit",
            "revoked",
            "cancelled",
            "source-replaced",
            "read-budget",
            "total-pressure",
            "producer-controls",
        ] {
            let cache = tempfile::tempdir().unwrap();
            fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env("OMNI_SNAPSHOT_MODE", mode)
                .env("OMNI_CACHE_DIR", cache.path())
                .output()
                .unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "actual native snapshot {mode}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let mode = mode.to_str().unwrap();
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let kb = KnowledgeBase::init(&home, "Synthetic native snapshot host").unwrap();
    let (service, owner, selection) = NativeLibraryService::ordinary_local(
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
    let _fixture_memory = ready(Reservation {
        memory_bytes: 64 * 1024 * 1024,
        temporary_bytes: 0,
    });
    let archive_path = home.join("raw/frozen.zip");
    let (generation, snapshot, public) = package(root.path(), &archive_path, mode);
    let history = history(root.path(), &home);
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let command = wire::decode_request(
        &serde_json::to_vec(
            &serde_json::json!({"wire_version":1,"request_id":"native-snapshot",
        "command":{"op":"open_frozen","path":"raw/frozen.zip"}}),
        )
        .unwrap(),
    )
    .unwrap()
    .command;
    let approval = super::publication_tests::approve(&owner, &session, &command, true).unwrap();
    let permit = service
        .native_scope(&session, &command, Some(approval))
        .unwrap();
    let source = service
        .source(
            &permit,
            &wire::RelativePath::new("raw/frozen.zip").unwrap(),
            job_budget(),
        )
        .unwrap();
    let processing = source
        .processing(
            digest(b"actual native verified snapshot and stored query"),
            digest(include_bytes!("activation_tests.rs")),
        )
        .unwrap();
    let key = OwnedKey::new(processing.key());
    let history_owner = NativeHistoryOwner::select(&history, &key.context, &key.library).unwrap();
    let known = if matches!(mode, "unknown" | "unknown-explicit") {
        vec![]
    } else {
        vec![KnownSigner::selected("snapshot@example.invalid", &public).unwrap()]
    };
    let metadata = ready(NativeTrustedArchive::metadata_requirement(&known).unwrap());
    let limits = bounds();
    let ranges = StoredPlan {
        index_bytes: 2 * 1024 * 1024,
        table_bytes: 2 * 1024 * 1024,
    };
    let needed =
        NativeTrustedArchive::requirement(job_budget(), limits.budget.max_memory_bytes, ranges)
            .unwrap();
    let submitted = processing
        .submit(NativeJobPlan {
            operation_id: "native-snapshot",
            job_id: "native-snapshot",
            budget: job_budget(),
            reservation: needed,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let account = Account::current().unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    assert_eq!(claim.job(), submitted.job);
    let mut pressure = (mode == "total-pressure").then(|| {
        let held = holding(Reservation {
            memory_bytes: account.status().unwrap().memory_available,
            temporary_bytes: 0,
        });
        assert_eq!(account.status().unwrap().memory_available, 0);
        held
    });
    let cancel = AtomicBool::new(false);
    let policy = if matches!(
        mode,
        "unsigned-explicit" | "unknown-explicit" | "invalid-explicit"
    ) {
        NativeSnapshotPolicy::UnsignedExplicit
    } else {
        NativeSnapshotPolicy::KnownSignerRequired
    };
    let verified = NativeTrustedArchive::verify(
        processing,
        claim.clone(),
        NativeSnapshotRequest {
            owner: &owner,
            selection: &selection,
            known,
            policy,
            history: &history,
            history_owner: &history_owner,
            bounds: &limits,
            cancel: &cancel,
        },
        metadata,
    );
    let rejected = match mode {
        "unknown" | "unknown-explicit" => Some(SignatureState::ValidUnknown),
        "invalid" | "invalid-explicit" | "wrong-namespace" => Some(SignatureState::Invalid),
        "unsigned-required" => Some(SignatureState::Unsigned),
        _ => None,
    };
    if let Some(expected) = rejected {
        let mut error = match verified {
            Err(error) => error,
            Ok(_) => panic!("unaccepted native signature state cannot activate a frozen reader"),
        };
        assert_eq!(error.verification().unwrap().state, expected);
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "trust refusal preserves its actual causing Claim"
        );
        error
            .collect()
            .unwrap_or_else(|cause| panic!("actual native verifier collection: {cause:#}"));
        let (processing, returned, _cause) = error.into_parts();
        assert!(
            Arc::ptr_eq(&returned, &claim),
            "the exact original Arc Claim must survive owning refusal"
        );
        account
            .queue()
            .settle(&returned, processing.key(), Outcome::Failed)
            .unwrap();
        drop(processing);
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
        return;
    }
    let archive = verified
        .unwrap_or_else(|error| panic!("actual native signed archive: {:#}", error.cause()));
    assert_eq!(
        archive.snapshot_id(),
        &snapshot,
        "trust binds the producer's independently hashed canonical manifest"
    );
    assert_eq!(
        archive.verification().state,
        if mode == "unsigned-explicit" {
            SignatureState::Unsigned
        } else {
            SignatureState::ValidKnown
        }
    );
    archive.check().unwrap();
    if pressure.is_some() {
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "constructor must borrow the complete prepaid Claim under total account pressure"
        );
    }
    if mode == "read-budget" {
        assert!(
            ArchiveSource::admit_read(archive.as_ref(), claim.budget().max_bytes).is_err(),
            "constructor reads already consume the SAME aggregate source read budget"
        );
        archive.check().unwrap();
    }
    let control = Control::new(claim.deadline());
    let work = Work::split_claim(&claim, key.key(), ranges.requirement().unwrap()).unwrap();
    let mut reader = archive
        .clone()
        .open_generation(
            NativeFrozenReadPlan {
                generation: &generation,
                scope: "snapshot-shard",
                prefix: "indexes/native",
                ranges,
                bounds: &limits,
            },
            work,
            &cancel,
            &control,
        )
        .unwrap();
    assert_eq!(reader.id(), generation);
    for (method, text) in [
        (Mode::Lexical, "alpha"),
        (Mode::Exact, "shared archive"),
        (Mode::Regex, "^alpha.*archive$"),
    ] {
        let result = reader
            .search(
                Query {
                    mode: method,
                    query: text.into(),
                    scopes: vec![view()],
                    limit: 10,
                },
                &control,
            )
            .unwrap();
        assert!(
            result.result().complete,
            "direct STORED query must report complete native coverage"
        );
        assert_eq!(result.result().hits.len(), 1);
        assert_eq!(result.result().hits[0].key.id, "snapshot-source");
        assert_eq!(
            result.result().hits[0].digest,
            hex::encode(Sha256::digest(TEXT))
        );
        assert_eq!(result.result().generations["snapshot-shard"], generation);
        result.check().unwrap();
        if pressure.is_some() {
            assert_eq!(
                account.status().unwrap().memory_available,
                0,
                "actual frozen open/query must use the existing complete paid reader lifetime"
            );
        }
        reader = result.into_generation();
    }
    let result = reader
        .search(
            Query {
                mode: Mode::Lexical,
                query: "alpha".into(),
                scopes: vec![view()],
                limit: 10,
            },
            &control,
        )
        .unwrap();
    if mode == "revoked" {
        owner.revoke(&selection).unwrap();
    }
    if mode == "cancelled" {
        account.queue().cancel(submitted.job, key.key()).unwrap();
    }
    if mode == "source-replaced" {
        let bytes = fs::read(&archive_path).unwrap();
        fs::rename(&archive_path, home.join("raw/fixture-original.zip")).unwrap();
        fs::write(&archive_path, bytes).unwrap();
    }
    if matches!(mode, "revoked" | "cancelled" | "source-replaced") {
        assert!(archive.check().is_err());
        assert!(
            result.check().is_err(),
            "actual source/Claim/native policy drift must block final archived result delivery"
        );
    } else {
        result.check().unwrap();
    }
    drop(result);
    drop(archive);
    if let Some(held) = &mut pressure {
        held.finish().unwrap();
    }
    let outcome = if mode == "cancelled" {
        Outcome::Cancelled
    } else {
        Outcome::Completed
    };
    account.queue().settle(&claim, key.key(), outcome).unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    assert!(!root.path().join("loose-kernel").exists());
    assert!(!root.path().join("loose-source").exists());
}
