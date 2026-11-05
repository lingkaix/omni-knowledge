//! The active native bound is a real owned-child resident observation, not RLIMIT_AS.
#![cfg(unix)]
use std::process::{Command, Stdio};

/// A real running worker exceeds its actual one-byte RSS promise and is natively collected.
#[test]
fn actual_live_owned_worker_exceeds_its_native_rss_promise_and_is_collected() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_retrieval-kernel"))
        .args([
            "service",
            "--sources",
            root.path().to_str().unwrap(),
            "--store",
            store.path().to_str().unwrap(),
        ])
        // A real parent baseline plus ONE byte for its actual live owned worker.
        .env(
            "OMNI_KERNEL_MEMORY_BYTES",
            (16 * 1024 * 1024 + 1u64).to_string(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Keep the actual input open: the pure worker is LIVE, not merely an EOF/exit-status fixture.
    let input = child.stdin.take().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            drop(input);
            panic!("native RSS controller failed to settle its live worker within ten seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    drop(input);
    assert!(!output.status.success());
    let evidence = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(evidence.contains("resource-limited"), "{evidence}");
    assert!(
        evidence.contains("RSS") && evidence.contains("exceeded admitted"),
        "{evidence}"
    );
    assert!(evidence.contains("rss_budget_bytes=1"), "{evidence}");
}

/// Test-only exact-PID containment on assertion failure; forced kill is never a passing shutdown observation.
#[cfg(target_os = "linux")]
struct Subject(std::process::Child);
#[cfg(target_os = "linux")]
impl Drop for Subject {
    /// Reap only this test's parent process if its ordinary bounded check did not finish.
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Controlled EOF, failed output and SIGTERM all collect the real live worker before the parent can exit.
#[cfg(target_os = "linux")]
#[test]
fn controlled_shutdown_collects_the_actual_worker_before_parent_exit() {
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};
    for trigger in ["eof", "output-error", "sigterm"] {
        let root = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let mut subject = Subject(
            Command::new(env!("CARGO_BIN_EXE_retrieval-kernel"))
                .arg("service")
                .arg("--sources")
                .arg(root.path())
                .arg("--store")
                .arg(store.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let parent = subject.0.id();
        let mut stderr = subject.0.stderr.take().unwrap();
        let flags = unsafe { libc::fcntl(stderr.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(stderr.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut evidence = String::new();
        let worker: u32 = loop {
            let mut bytes = [0; 4096];
            match stderr.read(&mut bytes) {
                Ok(n) => evidence.push_str(std::str::from_utf8(&bytes[..n]).unwrap()),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("native diagnostic read: {error}"),
            }
            if let Some(pid) = evidence.lines().find_map(|line| {
                line.strip_prefix("owned-kernel pid=")
                    .and_then(|line| line.split_whitespace().next())
                    .and_then(|pid| pid.parse().ok())
            }) {
                break pid;
            }
            assert!(
                Instant::now() < deadline,
                "missing native worker ownership: {evidence}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let identity = std::fs::read_to_string(format!("/proc/{worker}/stat")).unwrap();
        let start = identity
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .nth(19)
            .unwrap()
            .to_owned();
        match trigger {
            "eof" => drop(subject.0.stdin.take()),
            "output-error" => {
                drop(subject.0.stdout.take());
                subject.0.stdin.as_mut().unwrap().write_all(b"{\"version\":1,\"id\":1,\"op\":\"analyze\",\"query\":\"x\",\"title\":\"\",\"body\":\"\"}\n").unwrap();
            }
            "sigterm" => assert_eq!(unsafe { libc::kill(parent as i32, libc::SIGTERM) }, 0),
            _ => unreachable!(),
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = subject.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "controlled {trigger} failed to collect"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.success(), trigger == "eof");
        let same_worker = std::fs::read_to_string(format!("/proc/{worker}/stat"))
            .ok()
            .is_some_and(|stat| {
                stat.rsplit_once(')').unwrap().1.split_whitespace().nth(19) == Some(start.as_str())
            });
        assert!(
            !same_worker,
            "{trigger} parent exited before actual worker collection"
        );
        drop(stderr);
        drop(subject);
        eprintln!(
            "CONTROL {trigger} owner_pid={parent} worker_pid={worker} start={start} collected=true test_pipe_handles_closed=true"
        );
    }
}
