//! Real FD/range engine and ONE Core account; no archive, index, backend or numeric grant stand-in.
use knowledge_core::admission::{Account, Configuration, Decision, Origin, Scheduling};
use knowledge_core::jobs::Reservation;
use knowledge_retrieval::{ArchiveSource, Control, Error, HeldRangeReaders, Work};
use std::{
    fs::File,
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
struct Source {
    file: File,
    current: Arc<AtomicBool>,
}
impl ArchiveSource for Source {
    fn check(&self) -> Result<(), Error> {
        if self.current.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::Inventory)
        }
    }
    fn file(&self) -> Result<&File, Error> {
        self.check()?;
        Ok(&self.file)
    }
}
#[test]
fn returned_bytes_retain_exact_fd_credit_and_local_live_peak_until_last_slice() {
    let home = tempfile::tempdir().unwrap();
    std::fs::set_permissions(
        home.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let account = Account::start(
        File::open(home.path()).unwrap(),
        Configuration {
            memory_bytes: None,
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 16,
            max_records: 128,
            max_descriptor_bytes: 65536,
            per_mount: 1,
        },
    )
    .unwrap();
    let baseline = account.status().unwrap().memory_available;
    let window = 65536u64;
    let need = HeldRangeReaders::requirement(window).unwrap();
    let mut work = match Work::reserve(
        Reservation {
            temporary_bytes: window * 2,
            ..need
        },
        Origin::BeforeVerification,
    )
    .unwrap()
    {
        Decision::Ready(w) => w,
        Decision::Waiting(w) => panic!("actual fixture wait {w:?}"),
    };
    let mut writer = File::create(home.path().join("actual-native-component")).unwrap();
    writer
        .write_all(&vec![b'x'; (window * 2) as usize])
        .unwrap();
    writer.sync_all().unwrap();
    drop(writer);
    work.written(window * 2).unwrap();
    let current = Arc::new(AtomicBool::new(true));
    let source = Arc::new(Source {
        file: File::open(home.path().join("actual-native-component")).unwrap(),
        current: current.clone(),
    });
    let weak = Arc::downgrade(&source);
    let reserved = account.status().unwrap().reserved.memory_bytes;
    let work = match HeldRangeReaders::new_owning(
        work,
        window,
        Control::new(Instant::now() - Duration::from_secs(1)),
    ) {
        Err((work, _)) => work,
        Ok(_) => panic!("expired range factory admitted"),
    };
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        reserved,
        "failed factory must retain the actual prepaid Work for exact retry/rejoin"
    );
    let work = match HeldRangeReaders::new_owning(
        work,
        0,
        Control::new(Instant::now() + Duration::from_secs(10)),
    ) {
        Err((work, _)) => work,
        Ok(_) => panic!("zero range factory admitted"),
    };
    assert_eq!(work.remaining().unwrap().memory_bytes, need.memory_bytes);
    let readers = HeldRangeReaders::new(
        work,
        window,
        Control::new(Instant::now() + Duration::from_secs(10)),
    )
    .unwrap();
    let handle = readers.open(source.clone(), 7, window).unwrap();
    let mut buffers = Vec::new();
    for _ in 0..4 {
        buffers.push(handle.read_bytes(0..window as usize).unwrap());
    }
    assert_eq!(
        handle.read_bytes(0..1).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "a fifth byte cannot allocate beyond the existing paid peak"
    );
    let charged = account.status().unwrap().memory_available;
    assert_eq!(charged, baseline - need.memory_bytes);
    drop(buffers.pop());
    let again = handle.read_bytes(0..window as usize).unwrap();
    assert_eq!(
        account.status().unwrap().memory_available,
        charged,
        "local liveness return cannot refund/reserve Core credit"
    );
    let last = again.slice(13..19);
    drop(again);
    drop(buffers);
    current.store(false, Ordering::Release);
    assert!(handle.read_bytes(0..1).is_err());
    drop(source);
    drop(handle);
    let readers = match readers.into_work() {
        Err(owner) => owner,
        Ok(_) => panic!(
            "live returned bytes must return the actual owning refusal, never rejoinable Work"
        ),
    };
    assert_eq!(
        account.status().unwrap().memory_available,
        charged,
        "owning refusal keeps exact credit without global gap"
    );
    drop(readers);
    assert!(
        weak.upgrade().is_some(),
        "returned owned slice must retain the actual source descriptor and currentness owner"
    );
    assert_eq!(
        account.status().unwrap().memory_available,
        charged,
        "returned owned slice must retain exact prepaid credit after all handles die"
    );
    assert_eq!(&*last, b"xxxxxx");
    drop(last);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        account.status().unwrap().memory_available,
        baseline,
        "last actual bytes/FDs die before explicit Work finish"
    );
    let work = match Work::reserve(need, Origin::BeforeVerification).unwrap() {
        Decision::Ready(work) => work,
        Decision::Waiting(reason) => panic!("fixture waiting: {reason:?}"),
    };
    let readers = HeldRangeReaders::new(
        work,
        window,
        Control::new(Instant::now() + Duration::from_secs(10)),
    )
    .unwrap();
    let charged = account.status().unwrap().memory_available;
    let work = match readers.into_work() {
        Ok(work) => work,
        Err(_) => panic!("exclusive actual range resources must rejoin"),
    };
    assert_eq!(
        account.status().unwrap().memory_available,
        charged,
        "exact Work recovery never refunds then reserves"
    );
    assert_eq!(work.remaining().unwrap(), need);
    drop(work);
    assert_eq!(account.status().unwrap().memory_available, baseline);
}
