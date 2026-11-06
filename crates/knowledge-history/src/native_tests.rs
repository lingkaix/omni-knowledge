//! Real owned OS fixtures for the existing native executor, never substitutions for Git/signing results.
use super::*;
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Key, Limits, Outcome, Priority, Queue, Reservation, Submission};

/// Linux deterministic group/pipe fixture; native macOS supervision still requires its own qualification.
#[cfg(target_os = "linux")]
#[test]
fn native_blocked_input_and_actual_unsettled_lock_ownership() {
    let root = tempfile::tempdir().unwrap();
    let version =
        ContentDigest::new(digest(b"actual-owned-Python-group-and-pipe-fixture")).unwrap();
    let key = Key {
        context: "owned-native-fixture",
        library: "native",
        mount: None,
        source: "group-pipe-fixture",
        version: &version,
        recipe: &version,
        processor: &version,
    };
    let queue = Queue::new(Limits {
        max_pending: 2,
        max_records: 2,
        max_descriptor_bytes: 4096,
        per_mount: 2,
        memory_bytes: 128 << 20,
        temporary_bytes: 128 << 20,
    })
    .unwrap();
    queue
        .submit(Submission {
            operation_id: "native-owned-fixture",
            job_id: "native-owned-fixture",
            key,
            budget: Budget {
                max_files: 100,
                max_bytes: 524_288,
                max_millis: 1000,
                max_memory_bytes: 64 << 20,
            },
            reservation: Reservation {
                memory_bytes: 64 << 20,
                temporary_bytes: 64 << 20,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = queue.claim(Instant::now()).unwrap().unwrap();
    let runtime = native::Runtime::default();
    let mut blocked = Command::new("/usr/bin/python3");
    blocked
        .env_clear()
        .args(["-c", "import time; time.sleep(8)"]);
    let started = Instant::now();
    assert_eq!(
        runtime
            .execute(blocked, &vec![b'x'; 131_072], &Call::new(&claim).unwrap())
            .err()
            .unwrap(),
        Error::Deadline
    );
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(runtime.pending().unwrap().is_none());
    queue.settle(&claim, key, Outcome::Completed).unwrap();

    // Fresh actual Core claim, not a restarted deadline on the preceding native call.
    let second_key = Key {
        source: "actual-escaped-owned-io",
        ..key
    };
    queue
        .submit(Submission {
            operation_id: "native-unsettled-fixture",
            job_id: "native-unsettled-fixture",
            key: second_key,
            budget: Budget {
                max_files: 100,
                max_bytes: 524_288,
                max_millis: 1000,
                max_memory_bytes: 64 << 20,
            },
            reservation: Reservation {
                memory_bytes: 64 << 20,
                temporary_bytes: 64 << 20,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = queue.claim(Instant::now()).unwrap().unwrap();
    let runtime = native::Runtime::default();
    let call = Call::new(&claim).unwrap();
    let scratch = native::Scratch::new(root.path(), &call).unwrap();
    let retained_path = scratch.path().to_owned();
    let path = root.path().join("writer.lock");
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    FileExt::try_lock_exclusive(&file).unwrap();
    let guard = WriterLock(Some(OwnedWriterLock::new(file)), call.retention.clone());
    let ready = root.path().join("ready");
    let release = root.path().join("release");
    let script = "import os,pathlib,sys,time\nr,w=os.pipe()\np=os.fork()\nif p:\n os.close(w);os.read(r,1);os._exit(0)\nos.close(r);os.setsid();pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));os.write(w,b'x');os.close(w)\nend=time.monotonic()+8\nwhile time.monotonic()<end and not pathlib.Path(sys.argv[2]).exists(): time.sleep(.005)\nos._exit(0)\n";
    let mut command = Command::new("/usr/bin/python3");
    command
        .env_clear()
        .args(["-c", script])
        .arg(&ready)
        .arg(&release)
        .current_dir(scratch.path());
    let error = runtime.execute(command, b"", &call).err().unwrap();
    let Error::Unsettled(owned) = error else {
        panic!("actual escaped owned pipe must remain unresolved");
    };
    assert!(ready.exists());
    assert!(owned.inspect().unwrap().0.start.is_some());
    drop(guard);
    drop(scratch);
    assert!(retained_path.exists());
    assert!(!owned.inspect().unwrap().1.is_empty());
    let contender = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert_eq!(
        FileExt::try_lock_exclusive(&contender).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    // The error is an actual owner: dropping History/runtime cannot erase the child, pipes, lock or recognized artifacts.
    drop(runtime);
    assert_eq!(
        FileExt::try_lock_exclusive(&contender).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    fs::write(release, b"release exact owned fixture").unwrap();
    owned.collect().unwrap();
    FileExt::try_lock_exclusive(&contender).unwrap();
    FileExt::unlock(&contender).unwrap();
    // Only after native collection may this original claim settle or recognized artifacts be removed.
    queue
        .settle(&claim, second_key, Outcome::Completed)
        .unwrap();
    fs::remove_dir_all(retained_path).unwrap();
}

/// Actual A then B on one runtime: settled A idempotency must not settle/discard B or release B's unwind resources.
#[cfg(target_os = "linux")]
#[test]
fn stale_collector_cannot_settle_newer_owner_or_its_concurrent_unwind() {
    use std::sync::{Barrier, mpsc};
    // Distinct deciding controls: unwind alone, then a real native execution probe before unwind.
    let mut outcomes = Vec::new();
    for probe_execution in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let script = "import os,pathlib,sys,time\nr,w=os.pipe()\np=os.fork()\nif p:\n os.close(w);os.read(r,1);os._exit(0)\nos.close(r);os.setsid();pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));os.write(w,b'x');os.close(w)\nend=time.monotonic()+8\nwhile time.monotonic()<end and not pathlib.Path(sys.argv[2]).exists(): time.sleep(.005)\nos._exit(0)\n";
        let version = ContentDigest::new(digest(script.as_bytes())).unwrap();
        let key = Key {
            context: "owned-generation-regression",
            library: "native",
            mount: None,
            source: "actual-owned-generation-A",
            version: &version,
            recipe: &version,
            processor: &version,
        };
        let queue = Queue::new(Limits {
            max_pending: 3,
            max_records: 3,
            max_descriptor_bytes: 4096,
            per_mount: 3,
            memory_bytes: 128 << 20,
            temporary_bytes: 128 << 20,
        })
        .unwrap();
        let submit = |id: &str, key: Key<'_>| {
            queue
                .submit(Submission {
                    operation_id: id,
                    job_id: id,
                    key,
                    budget: Budget {
                        max_files: 100,
                        max_bytes: 524_288,
                        max_millis: 1000,
                        max_memory_bytes: 64 << 20,
                    },
                    reservation: Reservation {
                        memory_bytes: 64 << 20,
                        temporary_bytes: 64 << 20,
                    },
                    priority: Priority::Normal,
                    max_attempts: 1,
                })
                .unwrap();
            queue.claim(Instant::now()).unwrap().unwrap()
        };
        let runtime = native::Runtime::default();
        let a_claim = submit("actual-generation-A", key);
        let a_call = Call::new(&a_claim).unwrap();
        let a_scratch = native::Scratch::new(root.path(), &a_call).unwrap();
        let a_scratch_path = a_scratch.path().to_owned();
        let a_release = root.path().join("release-A");
        let mut command = Command::new("/usr/bin/python3");
        command
            .env_clear()
            .args(["-c", script])
            .arg(root.path().join("ready-A"))
            .arg(&a_release)
            .current_dir(a_scratch.path());
        let Error::Unsettled(a) = runtime.execute(command, b"", &a_call).err().unwrap() else {
            panic!("actual A must own its unsettled pipe");
        };
        fs::write(a_release, b"release actual A").unwrap();
        a.collect().unwrap();
        // Its caller/claim/scratch remain live until B is pending, proving guards are call-bound too.
        let b_key = Key {
            source: "actual-owned-generation-B",
            ..key
        };
        let b_claim = submit("actual-generation-B", b_key);
        let b_release = root.path().join("release-B");
        let writer = root.path().join("generation-B.lock");
        let (sender, receiver) = mpsc::channel();
        let unwind = Barrier::new(3);
        let (
            b,
            b_claim,
            b_scratch_path,
            probe_blocked,
            lock_retained,
            scratch_retained,
            registered,
            slot_retained,
        ) = std::thread::scope(|scope| {
            let runtime = &runtime;
            let root = root.path();
            let writer = &writer;
            let b_release = &b_release;
            let unwind = &unwind;
            let worker = scope.spawn(move || {
                let b_call = Call::new(&b_claim).unwrap();
                let scratch = native::Scratch::new(root, &b_call).unwrap();
                let path = scratch.path().to_owned();
                let file = OpenOptions::new()
                    .create_new(true)
                    .read(true)
                    .write(true)
                    .open(writer)
                    .unwrap();
                FileExt::try_lock_exclusive(&file).unwrap();
                let guard = WriterLock(Some(OwnedWriterLock::new(file)), b_call.retention.clone());
                let mut command = Command::new("/usr/bin/python3");
                command
                    .env_clear()
                    .args(["-c", script])
                    .arg(root.join("ready-B"))
                    .arg(b_release)
                    .current_dir(scratch.path());
                let Error::Unsettled(b) = runtime.execute(command, b"", &b_call).err().unwrap()
                else {
                    panic!("actual B must own its unsettled pipe");
                };
                sender.send((b, path)).unwrap();
                unwind.wait(); // Parent has decisively re-collected A while B's actual guards are still live.
                drop(guard);
                drop(scratch);
                b_claim
            });
            let (b, b_path) = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            a.collect().unwrap(); // Deciding stale A call, not a sleep/race lottery.
            drop(a_scratch); // Known A must not leak its directory into unrelated unsettled B.
            let older_scope_cleared =
                !a_scratch_path.exists() && !b.inspect().unwrap().1.contains(&a_scratch_path);
            queue.settle(&a_claim, key, Outcome::Completed).unwrap();
            let probe_blocked = if probe_execution {
                let probe_key = Key {
                    source: "actual-native-Git-probe",
                    ..key
                };
                let probe_claim = submit("actual-generation-probe", probe_key);
                let mut probe = Command::new("/usr/bin/git");
                probe.env_clear().arg("--version");
                let result = runtime.execute(probe, b"", &Call::new(&probe_claim).unwrap());
                let blocked = matches!(result, Err(Error::Unsettled(ref owner)) if owner == &b);
                queue
                    .settle(&probe_claim, probe_key, Outcome::Completed)
                    .unwrap();
                blocked
            } else {
                true
            };
            let stale = scope.spawn(|| {
                unwind.wait();
                for _ in 0..16 {
                    a.collect().unwrap();
                }
            });
            unwind.wait();
            let b_claim = worker.join().unwrap();
            stale.join().unwrap();
            let contender = OpenOptions::new()
                .read(true)
                .write(true)
                .open(writer)
                .unwrap();
            let lock_retained = match FileExt::try_lock_exclusive(&contender) {
                Err(error) => error.kind() == std::io::ErrorKind::WouldBlock,
                Ok(()) => {
                    FileExt::unlock(&contender).unwrap();
                    false
                }
            };
            let scratch_retained = b_path.exists();
            let registered = b.inspect().unwrap().1.contains(&b_path);
            let b_identity = b.inspect().unwrap().0;
            let slot_retained = runtime
                .pending()
                .unwrap()
                .is_some_and(|(id, _)| id == b_identity);
            (
                b,
                b_claim,
                b_path,
                probe_blocked && older_scope_cleared,
                lock_retained,
                scratch_retained,
                registered,
                slot_retained,
            )
        });
        // Cleanup uses the actual retained B, never the stale A or a rediscovered PID; assertions follow cleanup even on RED.
        fs::write(b_release, b"release actual B").unwrap();
        b.collect().unwrap();
        queue.settle(&b_claim, b_key, Outcome::Completed).unwrap();
        a.collect().unwrap();
        let contender = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&writer)
            .unwrap();
        FileExt::try_lock_exclusive(&contender).unwrap();
        FileExt::unlock(&contender).unwrap();
        if b_scratch_path.exists() {
            fs::remove_dir_all(b_scratch_path).unwrap();
        }
        outcomes.push((
            probe_execution,
            lock_retained,
            scratch_retained,
            registered,
            probe_blocked,
            slot_retained,
        ));
    }
    assert!(
        outcomes
            .iter()
            .all(|(_, lock, scratch, registered, blocked, slot)| *lock
                && *scratch
                && *registered
                && *blocked
                && *slot),
        "actual owner-bound outcomes (probe, lock, scratch, registered, blocked, slot): {outcomes:?}"
    );
}

/// Actual exact owned exit plus queued bytes is neither EOF nor a reason to signal an empty group.
#[test]
fn actual_exited_group_drains_queued_output_without_empty_group_signal() {
    let version = ContentDigest::new(digest(b"actual-exit-before-queued-drain")).unwrap();
    let key = Key {
        context: "actual-queued-native-exit",
        library: "native",
        mount: None,
        source: "owned-sh-pipe",
        version: &version,
        recipe: &version,
        processor: &version,
    };
    let queue = Queue::new(Limits {
        max_pending: 1,
        max_records: 1,
        max_descriptor_bytes: 4096,
        per_mount: 1,
        memory_bytes: 128 << 20,
        temporary_bytes: 128 << 20,
    })
    .unwrap();
    queue
        .submit(Submission {
            operation_id: "queued-native-exit",
            job_id: "queued-native-exit",
            key,
            budget: Budget {
                max_files: 4,
                max_bytes: 131072,
                max_millis: 10000,
                max_memory_bytes: 64 << 20,
            },
            reservation: Reservation {
                memory_bytes: 64 << 20,
                temporary_bytes: 64 << 20,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = queue.claim(Instant::now()).unwrap().unwrap();
    let runtime = native::Runtime::default();
    let timing = native::QueuedExitFixture::start();
    let mut command = Command::new("/bin/sh");
    command.env_clear().args(["-c", "printf x; printf y >&2"]);
    let output = runtime.execute(command, b"", &Call::new(&claim).unwrap());
    // Always complete actual native collection before deciding the signal oracle.
    assert!(
        runtime.pending().unwrap().is_none(),
        "an unresolved actual owner is not settlement"
    );
    assert_eq!(
        timing.empty_signals(),
        0,
        "an exact exited/empty group with queued output must be drained, not signalled"
    );
    let output = output.unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"x");
    assert_eq!(output.stderr, b"y");
    drop(output);
    drop(timing);
    queue.settle(&claim, key, Outcome::Completed).unwrap();
}
