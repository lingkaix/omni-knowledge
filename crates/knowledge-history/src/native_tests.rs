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
    let scratch = native::Scratch::new(root.path(), &runtime).unwrap();
    let retained_path = scratch.path().to_owned();
    let path = root.path().join("writer.lock");
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    FileExt::try_lock_exclusive(&file).unwrap();
    let guard = WriterLock(Some(OwnedWriterLock::new(file)), &runtime);
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
    let error = runtime
        .execute(command, b"", &Call::new(&claim).unwrap())
        .err()
        .unwrap();
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
