//! Native allocator control for duplicate segment assembly, not embedding quality or physical RSS.
use knowledge_core::admission::{Account, Configuration, Decision, Origin, Scheduling};
use knowledge_core::jobs::Reservation;
use knowledge_search::{AccountGrant, ModelIdentity, VectorRow, VectorStore};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Observed;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Observed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(now, Ordering::SeqCst);
        // SAFETY: forward the allocator's unchanged native layout.
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        // SAFETY: this pointer/layout pair came from this allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Observed = Observed;

fn fixture_representation() -> knowledge_search::RepresentationIdentity {
    knowledge_search::RepresentationIdentity {
        digest: "11".repeat(32),
        processor: "22".repeat(32),
        modality: knowledge_search::VectorModality::Text,
    }
}

#[test]
fn real_disk_segment_build_does_not_duplicate_the_float_corpus() {
    const M: u64 = 1024 * 1024;
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let account = Account::start(
        File::open(cache.path()).unwrap(),
        Configuration {
            memory_bytes: Some(512 * M),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 8,
            max_records: 32,
            max_descriptor_bytes: M as usize,
            per_mount: 2,
        },
    )
    .unwrap();
    // Reserve the ENTIRE simultaneous source+builder need before creating vector inputs.
    let mut inputs = match account
        .reserve(
            Reservation {
                memory_bytes: 64 * M,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(reason) => panic!("source fixture waiting: {reason:?}"),
    };
    let grant = AccountGrant::reserve(
        Reservation {
            memory_bytes: 128 * M,
            temporary_bytes: 64 * M,
        },
        Origin::BeforeVerification,
    )
    .unwrap();
    let rows: Vec<_> = (0..8192)
        .map(|number| {
            let mut vector = vec![0.0; 1024];
            vector[number % 1024] = 1.0;
            VectorRow {
                representation: fixture_representation(),
                source_id: format!("source-{number}"),
                source_digest: "d".repeat(64),
                locator: "line:1".into(),
                vector,
            }
        })
        .collect();
    let identity = ModelIdentity {
        provider: "numeric-control".into(),
        model: "precomputed-control".into(),
        revision: "immutable-control-1".into(),
        dimension: 1024,
        metric: knowledge_search::VectorMetric::Cosine,
        normalization: knowledge_search::VectorNormalization::Raw,
        package_fingerprint: "c".repeat(64),
        embedding_space: "numeric-control-space".into(),
        preprocessing: "none".into(),
    };
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    let built = VectorStore::build(root.path(), &identity, &rows, &grant, None).unwrap();
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    assert_eq!(
        fs::metadata(root.path().join(built.generation()).join("segment.bin"))
            .unwrap()
            .len(),
        32 * M
    );
    eprintln!("vector assembly observed allocation above live inputs: {peak} bytes");
    assert!(
        peak < 16 * M as usize,
        "32 MiB source corpus must not acquire a second whole-segment buffer: {peak}"
    );
    drop(built);
    drop(rows);
    drop(identity);
    drop(grant);
    inputs.finish().unwrap();
}
