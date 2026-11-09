//! Public writer-lifetime regression derived from Storage / Mutations.
//!
//! This detects an existing defect on OLD; it is not a passing preservation replacement.
//! Only the external provider and unrelated process timing are controlled by the fixture.
use omni_knowledge::{IndexOptions, KnowledgeBase};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

unsafe extern "C" {
    /// Read at most count bytes using the async-signal-safe POSIX operation.
    fn read(fd: std::ffi::c_int, buf: *mut std::ffi::c_void, count: usize) -> isize;
    /// Write at most count bytes using the async-signal-safe POSIX operation.
    fn write(fd: std::ffi::c_int, buf: *const std::ffi::c_void, count: usize) -> isize;
}

/// A completed operation must release its writer even while an unrelated child awaits exec.
/// The storage mutation contract permits refusal only while another library writer is active.
#[test]
fn completed_operation_releases_writer_before_unrelated_child_exec() {
    let home = tempfile::tempdir().unwrap();
    let kb = super::retained::legacy_init(home.path(), "Writer lifetime fixture").unwrap();
    std::fs::write(kb.root.join("raw/note.txt"), "writer lifetime\n").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut indexing = KnowledgeBase::open(&kb.root).unwrap();
    let config = indexing.config.embedding.as_mut().unwrap();
    config.endpoint = format!("http://{}", listener.local_addr().unwrap());
    config.timeout_seconds = 5;
    let job = std::thread::spawn(move || {
        indexing.index(&IndexOptions {
            embeddings: true,
            force: false,
        })
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut provider = loop {
        if let Ok((stream, _)) = listener.accept() {
            break stream;
        }
        assert!(
            Instant::now() < deadline,
            "index never reached external provider"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    provider
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    provider
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut header = vec![];
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0_u8];
        provider.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 8192);
    }
    // Observe public exclusion while the real owner has unfinished work.
    let active_writer_refused = kb
        .set_metadata("raw/note.txt", &["during".into()], None)
        .is_err();
    let (mut parent_gate, child_gate) = UnixStream::pair().unwrap();
    parent_gate
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    child_gate
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let spawning = std::thread::spawn(move || {
        let fd = child_gate.as_raw_fd();
        let mut command = Command::new("/usr/bin/true");
        // SAFETY: after fork, only POSIX read/write and allocation-free error construction run.
        // The inherited socket has a finite timeout; no Rust locks, destructors, or allocator run.
        unsafe {
            command.pre_exec(move || {
                let mut byte = 1_u8;
                if write(fd, (&byte as *const u8).cast(), 1) != 1
                    || read(fd, (&mut byte as *mut u8).cast(), 1) != 1
                {
                    return Err(std::io::Error::from_raw_os_error(5));
                }
                Ok(())
            });
        }
        let result = command.spawn();
        drop(child_gate);
        result
    });
    let mut ready = [0_u8];
    let child_ready = parent_gate.read_exact(&mut ready);
    let provider_response = provider.write_all(
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    drop(provider);
    let index_result = job.join();
    let released_result = kb.set_metadata("raw/note.txt", &["after".into()], None);
    let child_still_awaits_exec = !spawning.is_finished();
    // Reap the child before any deciding assertion can panic.
    let release_gate = parent_gate.write_all(&[1]);
    let child_status = spawning.join().unwrap().unwrap().wait().unwrap();
    let after_child_result = kb.set_metadata("raw/note.txt", &["after-child".into()], None);
    println!(
        "active_writer_refused={active_writer_refused}; child_still_awaits_exec={child_still_awaits_exec}; released_result={released_result:?}; after_child_result={after_child_result:?}"
    );
    assert!(active_writer_refused);
    child_ready.unwrap();
    provider_response.unwrap();
    assert!(index_result.unwrap().is_err());
    assert!(
        child_still_awaits_exec,
        "probe lost the fork-before-exec precondition"
    );
    release_gate.unwrap();
    assert!(child_status.success());
    assert!(after_child_result.is_ok());
    assert!(
        released_result.is_ok(),
        "completed writer must not depend on unrelated child exec: {released_result:?}"
    );
}
