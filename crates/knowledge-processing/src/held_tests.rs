//! Owned native fault control for the held-FD return path; this is not a format-success substitute.

use super::*;
use crate::admission;

/// Escaped real open I/O must retain the exact source/directory/output descriptors and all real costs until collection.
#[test]
#[cfg(target_os = "linux")]
fn unknown_native_io_retains_exact_fd_leases_and_recovers_explicitly() {
    let _gate = admission::TEST_GATE
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    admission::establish_process_account().unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = tempfile::tempfile().unwrap();
    let directory = File::open(root.path()).unwrap();
    let destination = create(&directory, "owned-partial.mp4").unwrap();
    let inode = destination.metadata().unwrap().ino();
    let marker = root.path().join("owned-escaped-pid");
    let before = admission::current().unwrap().status().unwrap().reserved;
    let mut funding = Funding::BeforeVerification;
    let mut credits = Credits::new(
        &mut funding,
        0,
        8192,
        crate::opaque::capture_memory(4096),
        Some(64 * 1024 * 1024),
        4096,
    )
    .unwrap();
    let charged = admission::current().unwrap().status().unwrap().reserved;
    let mut command = Command::new("python3");
    command.args(["-c", "import os,subprocess,sys; os.write(int(sys.argv[2]),b'x'); p=subprocess.Popen(['/bin/sleep','30'],start_new_session=True,close_fds=False); open(sys.argv[1],'w').write(str(p.pid))"]).arg(&marker).arg(destination.as_raw_fd().to_string());
    inherit(
        &mut command,
        &[&source, &directory, &destination],
        Some(&directory),
        None,
    );
    let started = std::time::Instant::now();
    let result = worker::run(
        command,
        None,
        4096,
        Duration::from_millis(250),
        &mut credits,
        &funding,
        Some(4096),
    );
    let pid: i32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
    // SAFETY: this exact PID was just created by the owned fixture and is still held open by its 30-second sleep.
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    assert!(pidfd >= 0);
    // SAFETY: uniquely owned successful pidfd, not a pathname/name-based kill target.
    let pidfd = unsafe { File::from_raw_fd(pidfd) };
    // Cleanup precedes deciding assertions; target only this pinned escaped fixture identity.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
    let error = result
        .err()
        .expect("escaped real I/O cannot be declared collected");
    let mut error = failed(
        error,
        Some(directory),
        vec![NativeArtifact {
            name: "owned-partial.mp4".into(),
            file: destination,
        }],
        vec![source],
        &mut credits,
    );
    assert!(started.elapsed() < Duration::from_secs(4));
    let pending = error.downcast_mut::<UnsettledChild>().unwrap();
    assert_eq!(pending.descriptors().len(), 3);
    assert_eq!(pending.descriptors()[2].metadata().unwrap().ino(), inode);
    assert_eq!(pending.descriptors()[2].metadata().unwrap().len(), 1);
    assert!(pending.take_descriptors().is_err());
    // Parent flags stayed private even while the child inherited these exact numbers.
    for file in pending.descriptors() {
        assert_ne!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        charged
    );
    let mut holdings = pending.collect().unwrap();
    let descriptors = pending.take_descriptors().unwrap();
    assert_eq!(descriptors[2].metadata().unwrap().ino(), inode);
    let length = descriptors[2].metadata().unwrap().len();
    // Drop native consumer records only after their actual generated delta is known.
    drop(descriptors);
    for holding in &mut holdings {
        if holding.remaining().unwrap().temporary_bytes > 0 {
            holding.written(length).unwrap();
        }
        holding.finish().unwrap();
    }
    assert_eq!(
        admission::current().unwrap().status().unwrap().reserved,
        before
    );
}
