//! A fresh native scope consumes actual claim credit, not a late global reservation or new intent.
use super::*;
use knowledge_core::{
    admission::{Decision, Origin},
    backend::Budget,
    data::ContentDigest,
    jobs::{Key, Outcome, Priority, Reservation, Submission},
};
use sha2::{Digest, Sha256};
use std::{
    os::unix::fs::MetadataExt,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

struct Pin {
    file: File,
    ended: Arc<AtomicUsize>,
}
impl Pin {
    fn current(&self) -> bool {
        self.file.metadata().is_ok_and(|m| m.is_file())
    }
}
impl Drop for Pin {
    fn drop(&mut self) {
        self.ended.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn actual_claim_funded_scopes_share_exact_command_credit_but_not_result_pins() {
    // This test deliberately drains ALL free credit and must own that entire account premise.
    if std::env::var_os("OMNI_FUNDED_SCOPE_CHILD").is_none() {
        let output=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","runtime::native_tests::funded::actual_claim_funded_scopes_share_exact_command_credit_but_not_result_pins","--nocapture"])
            .env("OMNI_FUNDED_SCOPE_CHILD","1").output().unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "the actual isolated fixture MUST execute, not a filtered zero-test success"
        );
        assert!(
            output.status.success(),
            "funded scope child: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let f = Fixture::new();
    let account = account();
    let mut fixture = match account
        .reserve(
            Reservation {
                memory_bytes: 128 * 1024,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        _ => panic!("fixture credit"),
    };
    let frame=serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"funded-scope","command":{"op":"process","library_id":"library","source_ids":["visible"],"recipe":"extract","budget":{"max_files":1,"max_bytes":"1048576","max_millis":30000,"max_memory_bytes":"8388608"},"operation_id":"funded-scope"}})).unwrap();
    let request = decode_request(&frame).unwrap();
    let runtime = Runtime::new(f.authority.clone());
    let parent = runtime
        .native_scope(&f.session, &request.command, None)
        .unwrap();
    let parent_ended = Arc::new(AtomicUsize::new(0));
    let parent_pin = Pin {
        file: File::open(f.directory.path().join("visible.txt")).unwrap(),
        ended: parent_ended.clone(),
    };
    parent
        .retain_check(Arc::new(move || parent_pin.current()))
        .unwrap();
    let version = ContentDigest::new(format!("{:x}", Sha256::digest(&frame))).unwrap();
    let processor = ContentDigest::new(format!(
        "{:x}",
        Sha256::digest(include_bytes!("authority.rs"))
    ))
    .unwrap();
    let metadata = File::open(f.directory.path()).unwrap().metadata().unwrap();
    let context = format!("actual-owner-root:{}:{}", metadata.dev(), metadata.ino());
    let key = Key {
        context: &context,
        library: "library",
        mount: None,
        source: "actual-admitted-process-frame",
        version: &version,
        recipe: &version,
        processor: &processor,
    };
    let budget = Budget {
        max_files: 1,
        max_bytes: 1024 * 1024,
        max_millis: 30000,
        max_memory_bytes: 8 * 1024 * 1024,
    };
    let submitted = account
        .submit(Submission {
            operation_id: "actual-funded-scope",
            job_id: "actual-funded-scope",
            key,
            budget,
            reservation: Reservation {
                memory_bytes: 8 * 1024 * 1024,
                temporary_bytes: 0,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), submitted.job);
    let status = runtime
        .native_scope(
            &f.session,
            &Command::LibraryStatus {
                library_id: id("library"),
            },
            None,
        )
        .unwrap();
    let status_available = account.status().unwrap().memory_available;
    assert!(
        matches!(
            status.fork_processing_scope(
                &claim,
                key,
                Reservation {
                    memory_bytes: 1024 * 1024,
                    temporary_bytes: 0
                }
            ),
            Err(ErrorCode::InvalidRequest)
        ),
        "metadata status is not a processing/query command"
    );
    assert_eq!(account.status().unwrap().memory_available, status_available);
    drop(status);
    let available = account.status().unwrap().memory_available;
    assert!(matches!(
        parent.fork_processing_scope(
            &claim,
            key,
            Reservation {
                memory_bytes: 1,
                temporary_bytes: 0
            }
        ),
        Err(ErrorCode::BudgetExhausted)
    ));
    assert_eq!(account.status().unwrap().memory_available, available);
    let mut pressure = match account
        .reserve(
            Reservation {
                memory_bytes: available,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        _ => panic!("actual complete pressure"),
    };
    let child = parent
        .fork_processing_scope(
            &claim,
            key,
            Reservation {
                memory_bytes: 1024 * 1024,
                temporary_bytes: 0,
            },
        )
        .unwrap();
    assert!(
        std::ptr::eq(
            parent.admitted_command().unwrap(),
            child.admitted_command().unwrap()
        ),
        "actual shared sealed command, not another decoded/cloned intention"
    );
    assert_eq!(account.status().unwrap().memory_available, 0);
    child.reserve_result(128 * 1024).unwrap();
    child
        .check_path(&id("library"), &RelativePath::new("visible.txt").unwrap())
        .unwrap();
    let ended = Arc::new(AtomicUsize::new(0));
    let pin = Pin {
        file: File::open(f.directory.path().join("visible.txt")).unwrap(),
        ended: ended.clone(),
    };
    child.retain_check(Arc::new(move || pin.current())).unwrap();
    parent.check().unwrap();
    let clone = child.clone();
    clone.reserve_result(400 * 1024).unwrap();
    assert_eq!(
        child.reserve_result(300 * 1024),
        Err(ErrorCode::BudgetExhausted),
        "scope clones cannot respend one loan or request global fallback"
    );
    child.reserve_result(1).unwrap(); // refused spend did not partially debit the original loan.
    let child = match child.rejoin_processing_scope(&claim, key) {
        Err(crate::authority::ScopeRejoinError::Retained {
            scope,
            code: ErrorCode::Busy,
        }) => *scope,
        _ => panic!("a live cloned owner must retain the actual scope/pins/credit"),
    };
    assert_eq!(ended.load(Ordering::SeqCst), 0);
    drop(clone);
    assert!(child.rejoin_processing_scope(&claim, key).is_ok());
    assert_eq!(ended.load(Ordering::SeqCst), 1);
    assert_eq!(
        account.status().unwrap().memory_available,
        0,
        "exact claim rejoin has NO global release/reserve gap"
    );
    // More than the entire claim divided by one scope: a bounded live window, not a corpus cap.
    for _ in 0..32 {
        let scope = parent
            .fork_processing_scope(
                &claim,
                key,
                Reservation {
                    memory_bytes: 1024 * 1024,
                    temporary_bytes: 0,
                },
            )
            .unwrap();
        scope.reserve_result(128 * 1024).unwrap();
        let pin = Pin {
            file: File::open(f.directory.path().join("visible.txt")).unwrap(),
            ended: ended.clone(),
        };
        scope.retain_check(Arc::new(move || pin.current())).unwrap();
        assert!(scope.rejoin_processing_scope(&claim, key).is_ok());
        assert_eq!(account.status().unwrap().memory_available, 0);
    }
    assert_eq!(ended.load(Ordering::SeqCst), 33);
    let child = parent
        .fork_processing_scope(
            &claim,
            key,
            Reservation {
                memory_bytes: 1024 * 1024,
                temporary_bytes: 0,
            },
        )
        .unwrap();
    let pin = Pin {
        file: File::open(f.directory.path().join("visible.txt")).unwrap(),
        ended: ended.clone(),
    };
    child.retain_check(Arc::new(move || pin.current())).unwrap();
    drop(parent);
    assert_eq!(
        parent_ended.load(Ordering::SeqCst),
        1,
        "fresh scopes share command credit, not old source/result FD pins"
    );
    assert_eq!(
        account.status().unwrap().memory_available,
        0,
        "the exact command's real funding survives its original scope"
    );
    child.admitted_command().unwrap();
    assert_eq!(ended.load(Ordering::SeqCst), 33);
    drop(child);
    assert_eq!(
        ended.load(Ordering::SeqCst),
        34,
        "independent result pins actually end with that scope"
    );
    assert!(
        account.status().unwrap().memory_available >= 64 * 1024 * 1024,
        "shared command data/credit ends only with its actual last owner"
    );
    pressure.finish().unwrap();
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    drop(frame);
    drop(request);
    drop(context);
    drop(version);
    drop(processor);
    fixture.finish().unwrap();
}
