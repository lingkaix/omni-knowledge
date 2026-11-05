//! Ordinary two-file restore controls for Core's examined-object ceiling.
use super::*;
use knowledge_package::Staged;

const FIRST: &[u8] = b"# First ordinary page\n";
const SECOND: &[u8] = b"# Second ordinary page\n";

/// Capture and stage two actual regular files with a roomy setup budget.
fn two_pages(root: &Path) -> Held<Staged> {
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let first = root.join("first.md");
    let second = root.join("second.md");
    fs::write(&first, FIRST).unwrap();
    fs::write(&second, SECOND).unwrap();
    let mut first = File::open(first).unwrap();
    let mut second = File::open(second).unwrap();
    let mut inputs = [
        Input {
            path: rel("pages/a.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut first,
        },
        Input {
            path: rel("pages/b.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut second,
        },
    ];
    let destination = root.join("two.zip");
    let mut request = base_capture(&private, &destination);
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    {
        let captured = run(&limits, &cancel, |work| {
            capture(&mut request, &mut inputs, work).unwrap()
        });
        assert!(captured.linked);
    }
    let mut archive = File::open(destination).unwrap();
    run(&limits, &cancel, |work| {
        stage(&mut archive, work, &private, "restore", &[]).unwrap()
    })
}

/// Construct a real no-clobber destination owner in this test's isolated directory.
fn home(root: &Path) -> Home {
    let root = root.join("home");
    fs::create_dir(&root).unwrap();
    Home {
        root,
        calls: Vec::new(),
    }
}

/// Verify both original and staged generations remain exact after success or refusal.
fn preserved(root: &Path) {
    assert_eq!(fs::read(root.join("first.md")).unwrap(), FIRST);
    assert_eq!(fs::read(root.join("second.md")).unwrap(), SECOND);
    assert_eq!(
        fs::read(root.join("private/restore/files/pages/a.md")).unwrap(),
        FIRST
    );
    assert_eq!(
        fs::read(root.join("private/restore/files/pages/b.md")).unwrap(),
        SECOND
    );
    assert!(root.join("private/restore/COMPLETE").is_file());
}

/// A roomy supplied ceiling permits both exact copies and accounts every native examination.
#[test]
fn two_file_restore_counts_recovery_sources_and_copies() {
    let fixture = Fixture::new("files-positive");
    let mut staged = two_pages(&fixture.0);
    let mut install = home(&fixture.0);
    let mut history = HistorySpy { calls: 0 };
    let mut limits = bounds(1_000_000);
    limits.budget.max_files = 100;
    let cancel = AtomicBool::new(false);
    let (result, files) = staged.with_work(&limits, &cancel, |staged, work| {
        let result = apply(staged, &mut install, &mut history, work);
        (result, work.files)
    });
    let result = result.unwrap();
    assert_eq!(result.state, ApplyState::Completed);
    assert_eq!(result.applied, [rel("pages/a.md"), rel("pages/b.md")]);
    assert_eq!(fs::read(install.root.join("pages/a.md")).unwrap(), FIRST);
    assert_eq!(fs::read(install.root.join("pages/b.md")).unwrap(), SECOND);
    // Two marker observations + MANIFEST/PLAN + two (two parents, file, hash, copy) passes.
    assert_eq!(files, 14);
    assert!(files <= limits.budget.max_files);
    preserved(&fixture.0);
}

/// One admitted marker exhausts max_files=1 before further examination or any destination copy.
#[test]
fn low_file_budget_refuses_before_recovery_and_never_overshoots() {
    let fixture = Fixture::new("files-refusal");
    let mut staged = two_pages(&fixture.0);
    let mut install = home(&fixture.0);
    let mut history = HistorySpy { calls: 0 };
    let mut limits = bounds(1_000_000);
    limits.budget.max_files = 1;
    let cancel = AtomicBool::new(false);
    staged.with_work(&limits, &cancel, |staged, work| {
        assert_eq!(
            apply(staged, &mut install, &mut history, work),
            Err(Error::Quota)
        );
        assert_eq!(work.files, 1);
        assert_eq!(work.read_bytes, 0);
        // Reusing the exhausted call cannot examine another unavailable or present object.
        assert_eq!(
            apply(staged, &mut install, &mut history, work),
            Err(Error::Quota)
        );
        assert_eq!(work.files, 1);
        assert_eq!(work.read_bytes, 0);
    });
    assert_eq!(fs::read_dir(&install.root).unwrap().count(), 0);
    preserved(&fixture.0);
}

/// Exhaustion after the second source hash stops delegation, not only the owner's later copy.
#[test]
fn exhausted_file_budget_stops_before_the_next_owner_effect() {
    let fixture = Fixture::new("files-delegation");
    let mut staged = two_pages(&fixture.0);
    let mut install = home(&fixture.0);
    let mut history = HistorySpy { calls: 0 };
    let mut limits = bounds(1_000_000);
    limits.budget.max_files = 13;
    let cancel = AtomicBool::new(false);
    let (result, files) = staged.with_work(&limits, &cancel, |staged, work| {
        let result = apply(staged, &mut install, &mut history, work);
        (result, work.files)
    });
    assert_eq!(result, Err(Error::Quota));
    assert_eq!(files, limits.budget.max_files);
    // Earlier saved paths are not rolled back; the second owner never creates its candidate.
    assert_eq!(fs::read(install.root.join("pages/a.md")).unwrap(), FIRST);
    assert!(!install.root.join("pages/b.md").exists());
    assert!(!install.root.join("pages/b.package-pending").exists());
    preserved(&fixture.0);
}
