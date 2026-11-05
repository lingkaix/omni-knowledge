//! Builder composition of actual files, shared-account claims, native generations and cancellation.
use knowledge_core::admission::{Account, Configuration, Decision, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_retrieval::{
    Content, Control, Document, Error, KernelAdapter, Mode, Query, ScopeView, SourceKey, Work,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

/// Conservative fixture runtime promise; not a measured RSS or a new format ceiling.
const RUNTIME_BYTES: u64 = 256 * 1024 * 1024;

/// Derive actual input/build recipe facts; a digest labels bytes, never authorization.
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Declare the real native UTF-8 source; kernel verification rereads the actual file.
fn view(bytes: &[u8]) -> ScopeView {
    ScopeView {
        scope: "selected-source-shard-0".into(),
        unavailable_objects: 0,
        documents: vec![Document {
            key: SourceKey {
                namespace: "native-test".into(),
                id: "source".into(),
            },
            revision: digest(bytes),
            digest: digest(bytes),
            size: bytes.len() as u64,
            name: "source.txt".into(),
            title: "Native kernel".into(),
            media_type: "text/plain".into(),
            content: Content::Text {
                path: "source.txt".into(),
                range: None,
            },
            text_ranges: None,
            language: Some("unicode".into()),
            metadata: BTreeMap::new(),
            locator: None,
        }],
    }
}
/// Require the real shared-account admission, distinguishing unavailable current resources from success.
fn reserve(reservation: Reservation) -> Work {
    match Work::reserve(reservation, Origin::BeforeVerification).unwrap() {
        Decision::Ready(work) => work,
        Decision::Waiting(reason) => panic!("fixture resources unavailable: {reason:?}"),
    }
}
/// Supply a finite actual deadline instead of a hidden fresh operation clock on every read.
fn control() -> Control {
    Control::new(Instant::now() + Duration::from_secs(60))
}
/// Find a retained generation by its public exact ID, observing actual native storage rather than source text.
fn retained(store: &std::path::Path, generation: &str) -> bool {
    fs::read_dir(store)
        .unwrap()
        .any(|entry| entry.unwrap().path().join(generation).is_dir())
}

/// Exercise real kernel publication/search/pins, current bytes, compatibility, cancellation and one account.
#[test]
fn native_generation_lifetimes_use_the_actual_process_account() {
    let cache = tempfile::tempdir().unwrap();
    fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let account = Account::start(
        File::open(cache.path()).unwrap(),
        Configuration {
            memory_bytes: Some(1024 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 16,
            max_records: 128,
            max_descriptor_bytes: 1024 * 1024,
            per_mount: 1,
        },
    )
    .unwrap();
    KernelAdapter::initialize_runtime().unwrap();
    KernelAdapter::initialize_runtime().unwrap();
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        RUNTIME_BYTES,
        "one shared runtime, never a host plus facade double charge"
    );
    let sources = tempfile::tempdir().unwrap();
    let original = b"alpha native generation\n";
    fs::write(sources.path().join("source.txt"), original).unwrap();
    let store = cache.path().join("kernel");
    let kernel = KernelAdapter::open(
        sources.path(),
        &store,
        "actual-consumer-lock-features-target-vendor-v1",
    )
    .unwrap();
    let requirement = KernelAdapter::publish_requirement(4096).unwrap();
    let version = ContentDigest::new(digest(original)).unwrap();
    let recipe = ContentDigest::new(digest(b"actual native lexical publication recipe")).unwrap();
    let processor =
        ContentDigest::new(digest(b"actual-consumer-lock-features-target-vendor-v1")).unwrap();
    let key = Key {
        context: "owned-native-test",
        library: "selected-library",
        mount: None,
        source: "source",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    let job_reservation = Reservation {
        memory_bytes: requirement.memory_bytes + KernelAdapter::query_requirement().memory_bytes,
        temporary_bytes: requirement.temporary_bytes,
    };
    let submitted = account
        .submit(Submission {
            operation_id: "publish-1",
            job_id: "publish-1-job",
            key,
            budget: Budget {
                max_files: 1,
                max_bytes: original.len() as u64,
                max_millis: 60_000,
                max_memory_bytes: job_reservation.memory_bytes,
            },
            reservation: job_reservation,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), submitted.job);
    let mut publication = Work::split_claim(&claim, key, requirement).unwrap();
    let first = kernel
        .publish(view(original), None, &mut publication, &control())
        .unwrap();
    drop(publication);
    let reader_work = Work::split_claim(&claim, key, KernelAdapter::query_requirement()).unwrap();
    let reader = kernel
        .pin("selected-source-shard-0", reader_work, &control())
        .unwrap();
    assert_eq!(reader.id(), first);
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        RUNTIME_BYTES + KernelAdapter::query_requirement().memory_bytes
    );
    let mut query_work = reserve(KernelAdapter::query_requirement());
    let result = kernel
        .search(
            Query {
                mode: Mode::Lexical,
                query: "alpha".into(),
                scopes: vec![view(original)],
                limit: 10,
            },
            &mut query_work,
            &control(),
        )
        .unwrap();
    assert!(result.complete);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.generations["selected-source-shard-0"], first);
    let stopped = control();
    stopped.cancel();
    assert!(matches!(
        kernel.search(
            Query {
                mode: Mode::Exact,
                query: "alpha".into(),
                scopes: vec![view(original)],
                limit: 10
            },
            &mut query_work,
            &stopped
        ),
        Err(Error::Kernel(_))
    ));
    let changed = b"beta replacement generation\n";
    fs::write(sources.path().join("source.txt"), changed).unwrap();
    let stale = kernel
        .search(
            Query {
                mode: Mode::Exact,
                query: "alpha".into(),
                scopes: vec![view(original)],
                limit: 10,
            },
            &mut query_work,
            &control(),
        )
        .unwrap();
    assert!(stale.hits.is_empty());
    assert!(!stale.complete);
    let mut publication = reserve(requirement);
    let second = kernel
        .publish(
            view(changed),
            Some(first.clone()),
            &mut publication,
            &control(),
        )
        .unwrap();
    assert_ne!(second, first);
    assert!(retained(&store, &first));
    let third_bytes = b"gamma third native generation\n";
    fs::write(sources.path().join("source.txt"), third_bytes).unwrap();
    let third = kernel
        .publish(
            view(third_bytes),
            Some(second),
            &mut publication,
            &control(),
        )
        .unwrap();
    assert!(
        retained(&store, &first),
        "actual native reader pin must prevent retirement"
    );
    let cancelled = control();
    cancelled.cancel();
    assert!(
        kernel
            .publish(
                view(third_bytes),
                Some(third.clone()),
                &mut publication,
                &cancelled
            )
            .is_err()
    );
    assert_eq!(
        kernel.generation("selected-source-shard-0").unwrap(),
        Some(third)
    );
    let incompatible = KernelAdapter::open(
        sources.path(),
        &store,
        "changed-consumer-lock-features-target-vendor-v2",
    )
    .unwrap();
    assert!(
        incompatible
            .search(
                Query {
                    mode: Mode::Lexical,
                    query: "gamma".into(),
                    scopes: vec![view(third_bytes)],
                    limit: 10
                },
                &mut query_work,
                &control()
            )
            .is_err()
    );
    drop(publication);
    drop(query_work);
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        RUNTIME_BYTES + KernelAdapter::query_requirement().memory_bytes
    );
    drop(reader);
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        RUNTIME_BYTES
    );
    account.queue().retire(submitted.job, key).unwrap();
    assert_eq!(account.status().unwrap().holdings, 1);
}
