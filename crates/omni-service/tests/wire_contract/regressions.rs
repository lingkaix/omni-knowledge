//! Owned-schema refusals and encoder allocation bounds.
//!
//! The service contract admits JSON objects for records and JSON strings for
//! declared string unions. The 47 command goldens remain the valid controls.
//! Allocation is observed only after the response fixture already exists.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use omni_service::{
    CodecError, Id, MAX_CONTROL_FRAME_BYTES, Response, Result as WireResult, decode_request,
    encode_response,
};

use crate::support::{CANARY, assert_input_free, refuse};

const BUDGET: &str =
    r#"{"max_files":1,"max_bytes":"8388608","max_millis":30000,"max_memory_bytes":"1048576"}"#;
const EXPECTED: &str = concat!(
    r#"{"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","#,
    r#""size_bytes":"1","modified_ns":"0","changed_ns":null,"file_id":null,"backend_version":null}"#
);

/// Additional encoder allocation may cover the 1 MiB frame and short-lived growth.
/// A full copy of a caller-owned scalar several times that size stays outside this limit.
const ENCODER_ALLOCATION_LIMIT: usize = 4 * MAX_CONTROL_FRAME_BYTES;
const OVERSIZED_ID_BYTES: usize = 16 * 1024 * 1024;
const ADMITTED_ID_BYTES: usize = 256 * 1024;

fn refuse_malformed(bytes: &str, canary: &str) {
    let err = refuse(bytes.as_bytes());
    assert_ne!(err, CodecError::FrameTooLarge);
    assert_ne!(err, CodecError::UnsupportedVersion);
    assert_input_free(&err, canary);
}

#[test]
fn request_array_is_not_an_object() {
    refuse_malformed(
        r#"[1,"wire-shape",{"op":"library_status","library_id":"synthetic"}]"#,
        "wire-shape",
    );
}

#[test]
fn source_ref_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"wire-shape","command":{"op":"source_details","source":["lib","src"]}}"#,
        "wire-shape",
    );
}

#[test]
fn history_mode_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"wire-shape","command":{"op":"library_create","path":"synthetic-not-opened","name":"Synthetic","history_mode":{"signed":null},"operation_id":"synthetic"}}"#,
        "wire-shape",
    );
}

#[test]
fn command_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":["library_list"]}"#,
        CANARY,
    );
}

#[test]
fn nested_image_query_source_ref_array_is_not_an_object() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"query","query":{{"library_ids":["syn-library"],"keywords":[],"methods":["semantic"],"required_tags":[],"case_sensitive":false,"limit":1,"budget":{BUDGET},"image_query":["lib","src"]}}}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn budget_array_is_not_an_object() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"verify_source","source":{{"library_id":"syn-library","source_id":"syn-source"}},"budget":[]}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn expected_version_array_is_not_an_object() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":[],"offset":"0","length":1}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn target_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"edit_text","target":[],"expected":null,"text":"synthetic","operation_id":"syn-op"}}"#,
        CANARY,
    );
}

#[test]
fn search_request_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"query","query":[]}}"#,
        CANARY,
    );
}

#[test]
fn package_selection_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"snapshot_plan","library_id":"syn-library","selection":[],"destination":"/synthetic/snapshot.zip"}}"#,
        CANARY,
    );
}

#[test]
fn mount_mapping_array_is_not_an_object() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"restore_plan","path":"synthetic-os-folder/archive.zip","destination":"synthetic-os-folder/restored","mount_map":[["syn-mount","synthetic-os-folder"]]}}"#,
        CANARY,
    );
}

#[test]
fn mount_backend_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"mount_attach","library_id":"syn-library","name":"Synthetic mount","backend":{"local":null},"location":"synthetic-os-folder","destination":null,"operation_id":"syn-op"}}"#,
        CANARY,
    );
}

#[test]
fn transfer_mode_object_is_not_a_string() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"transfer","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"destination":{{"library_id":"syn-library","mount_id":null,"path":"pages/index.md"}},"mode":{{"copy":null}},"operation_id":"syn-op"}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn verification_object_is_not_a_string() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"refresh","library_id":"syn-library","mount_ids":[],"verification":{{"stat_first":null}},"embeddings":false,"budget":{BUDGET},"operation_id":"syn-op"}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn recipe_object_is_not_a_string() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"process","library_id":"syn-library","source_ids":["syn-source"],"recipe":{{"extract":null}},"budget":{BUDGET},"operation_id":"syn-op"}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn sync_action_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"sync","library_id":"syn-library","action":{"fetch":null},"operation_id":"syn-op"}}"#,
        CANARY,
    );
}

#[test]
fn package_verification_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"package_inspect","path":"synthetic-os-folder/archive.zip","verification":{"manifest":null}}}"#,
        CANARY,
    );
}

#[test]
fn exposure_mode_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"exposure_set","library_id":"syn-library","expected_revision":"syn-revision","mode":{"read_only":null},"sensitive":false,"excluded_paths":[],"transports":["owner_ipc"],"operation_id":"syn-op"}}"#,
        CANARY,
    );
}

#[test]
fn transport_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"exposure_set","library_id":"syn-library","expected_revision":"syn-revision","mode":"read_only","sensitive":false,"excluded_paths":[],"transports":[{"owner_ipc":null}],"operation_id":"syn-op"}}"#,
        CANARY,
    );
}

#[test]
fn search_method_object_is_not_a_string() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"{CANARY}","command":{{"op":"query","query":{{"library_ids":["syn-library"],"keywords":["alpha"],"methods":[{{"bm25":null}}],"required_tags":[],"case_sensitive":false,"limit":1,"budget":{BUDGET},"image_query":null}}}}}}"#
    );
    refuse_malformed(&bytes, CANARY);
}

#[test]
fn package_preset_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"snapshot_plan","library_id":"syn-library","selection":{"preset":{"frozen":null},"source_ids":[],"page_paths":[],"originals":"references","include_indexes":false,"include_history":false,"include_journal":false,"require_complete":true},"destination":"/synthetic/snapshot.zip"}}"#,
        CANARY,
    );
}

#[test]
fn originals_object_is_not_a_string() {
    refuse_malformed(
        r#"{"wire_version":1,"request_id":"CANARY-wire-9f3c","command":{"op":"snapshot_plan","library_id":"syn-library","selection":{"preset":"frozen","source_ids":[],"page_paths":[],"originals":{"references":null},"include_indexes":false,"include_history":false,"include_journal":false,"require_complete":true},"destination":"/synthetic/snapshot.zip"}}"#,
        CANARY,
    );
}

#[derive(Clone, Copy)]
struct Probe {
    recording: bool,
    live: usize,
    peak: usize,
    total: usize,
}

thread_local! {
    static PROBE: Cell<Probe> = const {
        Cell::new(Probe {
            recording: false,
            live: 0,
            peak: 0,
            total: 0,
        })
    };
}

fn record_alloc(size: usize) {
    PROBE.with(|cell| {
        let mut probe = cell.get();
        if !probe.recording {
            return;
        }
        probe.live = probe.live.saturating_add(size);
        probe.total = probe.total.saturating_add(size);
        if probe.live > probe.peak {
            probe.peak = probe.live;
        }
        cell.set(probe);
    });
}

fn record_dealloc(size: usize) {
    PROBE.with(|cell| {
        let mut probe = cell.get();
        if !probe.recording {
            return;
        }
        probe.live = probe.live.saturating_sub(size);
        cell.set(probe);
    });
}

fn record_realloc(old: usize, new_size: usize) {
    if new_size > old {
        record_alloc(new_size - old);
    } else {
        record_dealloc(old - new_size);
    }
}

struct EncodeAllocProbe;

unsafe impl GlobalAlloc for EncodeAllocProbe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_dealloc(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            record_realloc(layout.size(), new_size);
        }
        new_ptr
    }
}

#[global_allocator]
static ENCODE_ALLOC_PROBE: EncodeAllocProbe = EncodeAllocProbe;

struct Sample {
    peak: usize,
    total: usize,
}

fn begin_encode_sample() {
    PROBE.with(|cell| {
        cell.set(Probe {
            recording: true,
            live: 0,
            peak: 0,
            total: 0,
        });
    });
}

fn end_encode_sample() -> Sample {
    PROBE.with(|cell| {
        let mut probe = cell.get();
        probe.recording = false;
        cell.set(probe);
        Sample {
            peak: probe.peak,
            total: probe.total,
        }
    })
}

fn libraries_response(request_id: Id) -> Response {
    Response {
        wire_version: 1,
        request_id,
        result: WireResult::Libraries { libraries: vec![] },
    }
}

fn outcome_label(outcome: &std::result::Result<Vec<u8>, CodecError>) -> String {
    match outcome {
        Ok(bytes) => format!("ok {}", bytes.len()),
        Err(err) => format!("err {err:?}"),
    }
}

fn observe(response: &Response) -> (std::result::Result<Vec<u8>, CodecError>, Sample) {
    begin_encode_sample();
    let outcome = encode_response(response);
    let sample = end_encode_sample();
    (outcome, sample)
}

#[test]
fn admitted_libraries_encode_stays_inside_the_frame() {
    let response = libraries_response(Id::new("syn-library").expect("fixture id"));
    let (outcome, sample) = observe(&response);
    let encoded = outcome.expect("ordinary libraries encode");
    assert!(encoded.len() <= MAX_CONTROL_FRAME_BYTES);
    assert!(sample.peak >= encoded.len());
    assert!(sample.peak <= ENCODER_ALLOCATION_LIMIT);
    assert!(sample.total <= ENCODER_ALLOCATION_LIMIT);
    let again = decode_request(
        br#"{"wire_version":1,"request_id":"syn-library-list","command":{"op":"library_list"}}"#,
    );
    assert!(again.is_ok(), "allocation probe changed ordinary decoding");
}

#[test]
fn admitted_medium_id_encode_is_visible_and_inside_the_frame() {
    let id = Id::new("b".repeat(ADMITTED_ID_BYTES)).expect("fixture id");
    assert_eq!(id.as_str().len(), ADMITTED_ID_BYTES);
    let response = libraries_response(id);
    let (outcome, sample) = observe(&response);
    let encoded = outcome.expect("admitted id encodes");
    assert!(encoded.len() > ADMITTED_ID_BYTES);
    assert!(encoded.len() <= MAX_CONTROL_FRAME_BYTES);
    assert!(
        sample.peak >= encoded.len(),
        "probe missed the encode allocation: peak {} total {} encoded {}",
        sample.peak,
        sample.total,
        encoded.len()
    );
    assert!(sample.peak <= ENCODER_ALLOCATION_LIMIT);
    assert!(sample.total <= ENCODER_ALLOCATION_LIMIT);
}

#[test]
fn oversized_libraries_id_does_not_allocate_a_full_caller_clone() {
    let id = Id::new("a".repeat(OVERSIZED_ID_BYTES)).expect("public Id has no length quota");
    let id_len = id.as_str().len();
    assert!(id_len >= OVERSIZED_ID_BYTES);
    assert!(id_len > 8 * MAX_CONTROL_FRAME_BYTES);
    assert!(ENCODER_ALLOCATION_LIMIT < id_len / 2);
    let response = libraries_response(id);
    // The fixture allocation above is outside the sample.
    let (outcome, sample) = observe(&response);
    let label = outcome_label(&outcome);
    assert!(
        sample.peak <= ENCODER_ALLOCATION_LIMIT,
        "encoder peak {} exceeded {ENCODER_ALLOCATION_LIMIT}; id_len {id_len}; total {}; {label}",
        sample.peak,
        sample.total
    );
    assert!(
        sample.total < id_len,
        "encoder total {} copied the caller id {id_len}; peak {}; {label}",
        sample.total,
        sample.peak
    );
}
