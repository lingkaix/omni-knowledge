//! Isolated Apple C ABI. Native objects and callback context remain owned until process exit.
//!
//! ABI: FSEvents.h (macOS 10.15 SDK), CFString.h, CFArray.h, CFUUID.h and CFRunLoop.h.
//! No dispatch queue or Rust worker is created. The stream delivers callbacks synchronously
//! on this helper's one scheduled runloop. Parent wait, not Stop/Invalidate, proves quiescence.
use super::protocol::{Frame, READY};
use std::{
    ffi::{CStr, c_void},
    os::fd::FromRawFd,
    os::unix::fs::MetadataExt,
    time::{Duration, Instant},
};

/// Opaque CF/stream references are never dereferenced by Rust.
type Ref = *const c_void;
/// Context layout from FSEventStreamContext: CFIndex, info, retain, release, copyDescription.
#[repr(C)]
struct Context {
    /// Native ABI version, always zero.
    version: isize,
    /// Leaked process-owned State; never a parent Rust reference.
    info: *mut c_void,
    /// Optional native retain callback, unused because State lasts until process exit.
    retain: Option<unsafe extern "C" fn(Ref) -> Ref>,
    /// Optional native release callback, deliberately unused at the process lifetime boundary.
    release: Option<unsafe extern "C" fn(Ref)>,
    /// Optional diagnostic callback; no path/content description is provided.
    describe: Option<unsafe extern "C" fn(Ref) -> Ref>,
}
/// CFUUIDBytes consists of sixteen UInt8 fields, with byte alignment.
#[repr(C)]
struct Uuid {
    /// Exact CFUUID bytes returned by value, with no retained native pointer.
    bytes: [u8; 16],
}
/// Exact FSEventStreamCallback ABI: stream, info, count, paths, flags and IDs.
type Callback = unsafe extern "C" fn(Ref, *mut c_void, usize, *mut c_void, *const u32, *const u64);
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    /// Copy the current device event-store UUID; null means it cannot be established.
    fn FSEventsCopyUUIDForDevice(device: libc::dev_t) -> Ref;
    /// Create a device-relative stream from live CF paths and copied context; null means setup failed.
    fn FSEventStreamCreateRelativeToDevice(
        allocator: Ref,
        callback: Callback,
        context: *mut Context,
        device: libc::dev_t,
        paths: Ref,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> Ref;
    /// Schedule the live stream on this thread's live runloop/mode; no dispatch worker is selected.
    fn FSEventStreamScheduleWithRunLoop(stream: Ref, runloop: Ref, mode: Ref);
    /// Start the scheduled stream; native Boolean zero means failure, not confirmed quiescence.
    fn FSEventStreamStart(stream: Ref) -> u8;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    /// Copy the sixteen bytes of a valid immutable CFUUID.
    fn CFUUIDGetUUIDBytes(uuid: Ref) -> Uuid;
    /// Release an owned native reference; used only for synchronous UUID observations here.
    fn CFRelease(object: Ref);
    /// Copy the length-delimited encoded bytes into a CF string; null means conversion/allocation failed.
    fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        count: isize,
        encoding: u32,
        external: u8,
    ) -> Ref;
    /// Create an array from count live values; null callbacks borrow elements retained until process exit.
    fn CFArrayCreate(allocator: Ref, values: *const Ref, count: isize, callbacks: Ref) -> Ref;
    /// Borrow this thread's native runloop.
    fn CFRunLoopGetCurrent() -> Ref;
    /// Service the valid mode for a finite interval; this does not preempt an individual native callback.
    fn CFRunLoopRunInMode(mode: Ref, seconds: f64, once: u8) -> i32;
    /// Borrow the framework's immutable default runloop mode.
    static kCFRunLoopDefaultMode: Ref;
}
/// Prepaid setup metadata plus a fixed callback aggregate; event paths are only borrowed during delivery.
struct State {
    /// Fixed metadata aggregate sent through the bounded transport.
    frame: Frame,
    /// Actual unsent native hint work, not a durable acknowledgement.
    pending: bool,
    /// Finite aggregate event/exclusion work remaining in this lease.
    remaining: u64,
    /// Finite owned-runtime deadline.
    deadline: Instant,
    /// Device-relative root prefix and current captured exclusions; never content authority.
    prefix: Vec<u8>,
    /// Bounded setup-only metadata, borrowed without allocation by the callback.
    excluded: Vec<Vec<u8>>,
}
/// Match only a complete path component, never a textual prefix alias.
fn within(path: &[u8], scope: &[u8]) -> bool {
    scope.is_empty()
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|p| p.starts_with(b"/"))
}
/// Bound callback work, preserve native loss, and never unwind or allocate across the C ABI.
unsafe extern "C" fn callback(
    _: Ref,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    ids: *const u64,
) {
    // SAFETY: info points to the leaked State below, live until process exit. Only this runloop accesses it.
    let s = unsafe { &mut *info.cast::<State>() };
    if flags.is_null()
        || ids.is_null()
        || count as u64 > s.remaining
        || Instant::now() >= s.deadline
    {
        s.frame.flags |= 1;
        s.pending = true;
        s.remaining = 0;
        return;
    }
    for i in 0..count {
        if s.remaining == 0 || Instant::now() >= s.deadline {
            s.remaining = 0;
            s.frame.flags |= 1;
            s.pending = true;
            break;
        }
        // SAFETY: FSEvents supplies count valid UInt32 flags and UInt64 IDs for this callback only.
        let flag = unsafe { *flags.add(i) };
        if flag & !super::protocol::KNOWN_FLAGS != 0 {
            s.frame.flags |= 1;
        }
        s.remaining -= 1;
        if flag == 0x10 {
            continue;
        } // HistoryDone is a control boundary, not a source-change hint.
        let mut excluded = false;
        if paths.is_null() {
            s.frame.flags |= 1;
        }
        if flag & super::protocol::GAP == 0 && !paths.is_null() {
            // SAFETY: without UseCFTypes the framework supplies count nul-terminated char* paths.
            let p = unsafe { *paths.cast::<*const libc::c_char>().add(i) };
            if !p.is_null() {
                let n = unsafe { libc::strnlen(p, libc::PATH_MAX as usize) };
                if n < libc::PATH_MAX as usize {
                    let path = unsafe { std::slice::from_raw_parts(p.cast::<u8>(), n) };
                    let path = path.strip_prefix(b"/").unwrap_or(path);
                    if !within(path, &s.prefix) {
                        s.frame.flags |= 1;
                    }
                    for scope in &s.excluded {
                        if s.remaining == 0 {
                            s.frame.flags |= 1;
                            break;
                        }
                        s.remaining -= 1;
                        if within(path, scope) {
                            excluded = true;
                            break;
                        }
                    }
                } else {
                    s.frame.flags |= 1;
                }
            } else {
                s.frame.flags |= 1;
            }
        }
        if !excluded || s.frame.flags & super::protocol::GAP != 0 {
            s.frame.event(flag, unsafe { *ids.add(i) });
            s.pending = true;
        }
    }
}
/// Write exactly one atomic nonblocking frame. Backpressure is a terminal gap, not silent event loss.
fn send(frame: Frame) -> bool {
    let b = frame.encode();
    // SAFETY: b is live for this synchronous write; stdout is the parent's sole pipe endpoint.
    unsafe { libc::write(1, b.as_ptr().cast(), b.len()) == b.len() as isize }
}
/// Observe this helper's opaque runtime using the existing sampled resident-set mechanism.
fn within_memory(limit: u64) -> bool {
    knowledge_processing::worker::resident_bytes(std::process::id()).is_ok_and(|n| n <= limit)
}
/// Recheck the actual device event-store UUID; an erased/replaced event store invalidates continuity.
fn same_volume(device: libc::dev_t, expected: [u8; 16]) -> bool {
    // SAFETY: Copy returns an owned immutable CFUUID. It is checked, read synchronously, then released.
    unsafe {
        let uuid = FSEventsCopyUUIDForDevice(device);
        if uuid.is_null() {
            return false;
        }
        let same = CFUUIDGetUUIDBytes(uuid).bytes == expected;
        CFRelease(uuid);
        same
    }
}
/// Construct a real device-relative stream from the inherited held root and run a finite lease.
/// No path or argument can replace that descriptor. Failure exits without a READY frame.
pub(super) fn run() -> i32 {
    let mut args = std::env::args().skip(1);
    let Some(memory) = args
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n >= 4 * 1024 * 1024)
    else {
        return 64;
    };
    let Some(millis) = args
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n != 0)
    else {
        return 64;
    };
    let Some(work) = args
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n != 0)
    else {
        return 64;
    };
    if !within_memory(memory) {
        return 65;
    }
    let Some(deadline) = Instant::now().checked_add(Duration::from_millis(millis)) else {
        return 64;
    };
    // SAFETY: the parent supplied its selected directory as this process's owned stdin descriptor.
    let root = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
    let Ok(meta) = root.metadata() else {
        return 66;
    };
    if !meta.is_dir() {
        return 66;
    }
    let mut path = [0u8; libc::PATH_MAX as usize];
    // SAFETY: Darwin F_GETPATH_NOFIRMLINK (fcntl.h, 102) writes at most PATH_MAX bytes.
    // The physical Data-volume path is required on APFS; the user-visible firmlink spelling is not device-relative.
    if unsafe { libc::fcntl(0, 102, path.as_mut_ptr()) } != 0 {
        return 66;
    }
    let Ok(path) = CStr::from_bytes_until_nul(&path) else {
        return 66;
    };
    // SAFETY: zero-initialized statfs has no Rust invariants; fstatfs writes its full native layout.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(0, &mut fs) } != 0 {
        return 66;
    }
    // SAFETY: fstatfs returns a nul-terminated fixed-size mount path.
    let mount = unsafe { CStr::from_ptr(fs.f_mntonname.as_ptr()) }.to_bytes();
    let absolute = path.to_bytes();
    let Some(relative) = super::protocol::relative_root(absolute, mount) else {
        return 66;
    };
    // SAFETY: all CF pointers below are checked before use and retained until process exit.
    // The array does not retain/release its string; that string has the same process lifetime.
    let (uuid, paths) = unsafe {
        let uuid = FSEventsCopyUUIDForDevice(meta.dev() as libc::dev_t);
        let string = CFStringCreateWithBytes(
            std::ptr::null(),
            relative.as_ptr(),
            relative.len() as isize,
            0x08000100,
            0,
        );
        if uuid.is_null() || string.is_null() {
            return 67;
        }
        let paths = CFArrayCreate(std::ptr::null(), &string, 1, std::ptr::null());
        if paths.is_null() {
            return 67;
        }
        (CFUUIDGetUUIDBytes(uuid).bytes, paths)
    };
    if uuid == [0; 16] {
        return 67;
    }
    let prefix = relative.strip_prefix(b"/").unwrap_or(relative).to_vec();
    let mut excluded = Vec::new();
    let mut metadata = prefix.len() as u64;
    for scope in args {
        if knowledge_core::data::RelativePath::new(&scope).is_err() {
            return 64;
        }
        metadata = match metadata.checked_add(prefix.len() as u64 + scope.len() as u64 + 64) {
            Some(n) if n <= memory / 8 && (excluded.len() as u64) < work => n,
            _ => return 65,
        };
        let mut path = prefix.clone();
        if !path.is_empty() {
            path.push(b'/');
        }
        path.extend_from_slice(scope.as_bytes());
        excluded.push(path);
    }
    let state = Box::into_raw(Box::new(State {
        frame: Frame {
            device: meta.dev(),
            root: meta.ino(),
            volume: uuid,
            event_id: 0,
            flags: READY,
        },
        pending: false,
        remaining: work,
        deadline,
        prefix,
        excluded,
    }));
    let context = Box::into_raw(Box::new(Context {
        version: 0,
        info: state.cast(),
        retain: None,
        release: None,
        describe: None,
    }));
    // SAFETY: immutable CF path array and leaked callback State remain live. SinceNow forces catch-up.
    // NoDefer | WatchRoot | FileEvents delivers precise hints, including explicit root-change flags.
    let stream = unsafe {
        FSEventStreamCreateRelativeToDevice(
            std::ptr::null(),
            callback,
            context,
            meta.dev() as libc::dev_t,
            paths,
            u64::MAX,
            0.05,
            0x16,
        )
    };
    if stream.is_null() {
        return 67;
    }
    // SAFETY: this stream is scheduled ONLY on this thread's runloop, with a live native default mode.
    unsafe {
        FSEventStreamScheduleWithRunLoop(stream, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);
    }
    if unsafe { FSEventStreamStart(stream) } == 0 {
        return 67;
    }
    // SAFETY: fcntl changes only this child's output pipe, not any caller-owned root flags.
    let flags = unsafe { libc::fcntl(1, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(1, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return 68;
    }
    // SAFETY: State remains allocated until exit. Access occurs only outside callback/runloop execution.
    if !within_memory(memory) || !send(unsafe { (*state).frame }) {
        return 68;
    }
    unsafe {
        (*state).frame.flags = 0;
        (*state).pending = false;
    }
    while Instant::now() < deadline && unsafe { (*state).remaining } != 0 {
        if !within_memory(memory) {
            return 69;
        }
        if !same_volume(meta.dev() as libc::dev_t, uuid) {
            return 70;
        }
        // SAFETY: synchronous bounded runloop service; no Rust reference is used during callback delivery.
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, 1);
        }
        // SAFETY: the synchronous runloop call returned; there is no concurrent State access.
        let s = unsafe { &mut *state };
        if s.pending {
            if !send(s.frame) {
                return 68;
            }
            s.pending = false;
            s.frame.flags = 0;
        }
    }
    // Do not free State/CF objects here. main immediately exits, and the parent must wait before settlement.
    // Keep the inherited selected root live until that same process boundary.
    0
}
