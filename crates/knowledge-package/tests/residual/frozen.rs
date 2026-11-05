//! Public held-container controls use real native files and native ZIP capture.
use super::super::*;
use knowledge_package::FrozenArchive;

/// Capture ordinary native bytes with the real writer; the reader must not extract them.
fn stored_fixture(root: &Path, job: &mut Work<'_>) -> PathBuf {
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let source = root.join("source.bin");
    fs::write(&source, b"needle\0binary\xff").unwrap();
    let mut file = File::open(source).unwrap();
    let dest = root.join("frozen.zip");
    let mut request = base_capture(&private, &dest);
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("m".into()),
        source_id: Some("s".into()),
        expected: None,
        reader: &mut file,
    }];
    capture(&mut request, &mut inputs, job).unwrap();
    dest
}

/// Positional reads cannot expose a neighbouring ZIP record or seek outside a member.
#[test]
fn stored_ranges_are_verified_bounded_and_not_extracted() {
    let root = dir("stored");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let path = stored_fixture(&root, job);
        let mut frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
        assert_eq!(frozen.manifest().entries.len(), 1);
        frozen
            .with_stored(&rel("media/a.bin"), |range| {
                let mut bytes = [0u8; 64];
                assert_eq!(range.read_at(0, &mut bytes)?, 14);
                assert_eq!(&bytes[..14], b"needle\0binary\xff");
                assert_eq!(range.read_at(14, &mut bytes)?, 0);
                assert_eq!(range.read_at(15, &mut bytes), Err(Error::Invalid));
                assert!(range.seek(SeekFrom::End(1)).is_err());
                assert!(range.seek(SeekFrom::Current(-1)).is_err());
                Ok(())
            })
            .unwrap();
        assert!(!root.join("media").exists());
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Native writes during a callback withhold success even if the callback does not read again.
#[test]
fn mutation_during_callback_refuses_the_result() {
    let root = dir("stored-drift");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let path = stored_fixture(&root, job);
        let mut frozen = FrozenArchive::open(File::open(&path).unwrap(), job).unwrap();
        let result = frozen.with_stored(&rel("media/a.bin"), |range| {
            let mut writer = OpenOptions::new().write(true).open(&path).unwrap();
            writer.seek(SeekFrom::Start(range.extent().offset)).unwrap();
            writer.write_all(b"changed").unwrap();
            writer.sync_all().unwrap();
            Ok(())
        });
        assert_eq!(result, Err(Error::Drift));
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Replacing a pathname cannot redirect the already verified descriptor to new bytes.
#[test]
fn replacement_never_mixes_container_versions() {
    let root = dir("stored-replaced");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let path = stored_fixture(&root, job);
        let mut frozen = FrozenArchive::open(File::open(&path).unwrap(), job).unwrap();
        fs::rename(&path, root.join("previous.zip")).unwrap();
        fs::write(&path, b"replacement must never be read").unwrap();
        let result = frozen.with_stored(&rel("media/a.bin"), |range| {
            let mut bytes = [0u8; 6];
            range.read_at(0, &mut bytes)?;
            assert_eq!(&bytes, b"needle");
            Ok(())
        });
        // Rename can update Unix ctime. Conservative refusal is valid, redirection is not.
        assert!(result == Ok(()) || result == Err(Error::Drift));
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Cancellation after opening prevents native callback access without releasing the pin early.
#[test]
fn stored_reader_obeys_live_cancellation() {
    let root = dir("stored-cancel");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let held = run(&limits, &cancel, |job| {
        let path = stored_fixture(&root, job);
        let mut frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            frozen.with_stored(&rel("media/a.bin"), |_| panic!("cancelled callback")),
            Err::<(), _>(Error::Cancelled)
        );
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}
