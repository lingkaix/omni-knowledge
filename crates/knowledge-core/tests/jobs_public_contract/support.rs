//! Fixtures for the public cooperative queue. No processor or backend is started.

use std::cell::Cell;
use std::time::Instant;

use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Limits, Priority, Queue, Reservation, Submission, Usage};

pub const CANARY: &str = "CANARY-jobs-public-9c2e";

thread_local! {
    static WINDOW: Cell<Window> = const { Cell::new(Window { armed: false, bytes: 0 }) };
}

#[derive(Clone, Copy)]
struct Window {
    armed: bool,
    bytes: usize,
}

/// Record one successful System allocation on this thread.
///
/// The counter is a const thread-local, so the hook does not allocate.
/// It observes the calling thread's allocator traffic only.
pub fn note_allocation(size: usize) {
    WINDOW.with(|slot| {
        let mut window = slot.get();
        if window.armed {
            window.bytes = window.bytes.saturating_add(size);
            slot.set(window);
        }
    });
}

/// Initialize the thread-local window before a measured call.
pub fn warmup_window() {
    WINDOW.with(|slot| {
        let window = slot.get();
        std::hint::black_box(window.bytes);
    });
}

/// Arm the calling thread, run `body`, and return its gross allocated bytes.
pub fn allocated_during<T>(body: impl FnOnce() -> T) -> (T, usize) {
    WINDOW.with(|slot| {
        slot.set(Window {
            armed: true,
            bytes: 0,
        });
    });
    let value = body();
    let bytes = WINDOW.with(|slot| {
        let mut window = slot.get();
        let bytes = window.bytes;
        window.armed = false;
        window.bytes = 0;
        slot.set(window);
        bytes
    });
    (value, bytes)
}

pub fn digest(seed: u8) -> ContentDigest {
    ContentDigest::new(format!("{seed:02x}").repeat(32)).expect("64 lowercase hex digits")
}

pub fn open_limits() -> Limits {
    Limits {
        max_pending: 8,
        max_records: 16,
        max_descriptor_bytes: 65_536,
        per_mount: 8,
        memory_bytes: 1_048_576,
        temporary_bytes: 1_048_576,
    }
}

pub fn budget(max_millis: u64) -> Budget {
    Budget {
        max_files: 2,
        max_bytes: 4_096,
        max_millis,
        max_memory_bytes: 8_192,
    }
}

pub fn reservation(memory_bytes: u64, temporary_bytes: u64) -> Reservation {
    Reservation {
        memory_bytes,
        temporary_bytes,
    }
}

pub struct Ident {
    pub context: String,
    pub library: String,
    pub mount: Option<String>,
    pub source: String,
    pub operation_id: String,
    pub job_id: String,
    pub version: ContentDigest,
    pub recipe: ContentDigest,
    pub processor: ContentDigest,
    pub budget: Budget,
    pub reservation: Reservation,
    pub priority: Priority,
    pub max_attempts: u64,
}

impl Ident {
    pub fn new(label: &str) -> Self {
        Self {
            context: format!("context-{label}"),
            library: format!("library-{label}"),
            mount: Some(format!("mount-{label}")),
            source: format!("source-{label}"),
            operation_id: format!("operation-{label}"),
            job_id: format!("job-{label}"),
            version: digest(0x11),
            recipe: digest(0x22),
            processor: digest(0x33),
            budget: budget(60_000),
            reservation: reservation(100, 40),
            priority: Priority::Normal,
            max_attempts: 3,
        }
    }

    pub fn key(&self) -> Key<'_> {
        Key {
            context: &self.context,
            library: &self.library,
            mount: self.mount.as_deref(),
            source: &self.source,
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }

    pub fn submission(&self) -> Submission<'_> {
        Submission {
            operation_id: &self.operation_id,
            job_id: &self.job_id,
            key: self.key(),
            budget: self.budget,
            reservation: self.reservation,
            priority: self.priority,
            max_attempts: self.max_attempts,
        }
    }
}

pub fn queue(limits: Limits) -> Queue {
    Queue::new(limits).expect("queue limits")
}

pub fn usage(queue: &Queue) -> Usage {
    queue.usage().expect("usage")
}

pub fn now() -> Instant {
    Instant::now()
}

pub fn assert_input_free(err: &impl std::fmt::Debug, canary: &str) {
    let shown = format!("{err:?}");
    assert!(!shown.contains(canary), "diagnostic retained caller input");
}
