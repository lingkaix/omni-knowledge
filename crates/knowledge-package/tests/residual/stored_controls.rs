//! ZIP64 and examined-file controls through the compiled held-range API.
use super::super::*;
use knowledge_package::FrozenArchive;

/// Actual sparse offsets above 4 GiB remain addressable without reading or extracting the hole.
#[test]
fn held_stored_range_reads_real_zip64_extent_above_four_gib() {
    let root = dir("held-zip64");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let plain = root.join("plain.zip");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let source = root.join("source.bin");
    fs::write(&source, b"hi").unwrap();
    let mut file = File::open(source).unwrap();
    let mut request = base_capture(&private, &plain);
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("m".into()),
        source_id: Some("s".into()),
        expected: None,
        reader: &mut file,
    }];
    drop(run(&limits, &cancel, |job| {
        capture(&mut request, &mut inputs, job).unwrap()
    }));
    let mut zip = zip::ZipArchive::new(File::open(&plain).unwrap()).unwrap();
    let mut manifest = Vec::new();
    zip.by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut manifest)
        .unwrap();
    let sparse = root.join("sparse.zip");
    write_sparse_zip64(&sparse, &manifest, b"media/a.bin", b"hi");
    let logical = fs::metadata(&sparse).unwrap().len();
    assert!(logical > 1 << 32);
    let limits = bounds(logical);
    let held = run(&limits, &cancel, |job| {
        let mut frozen = FrozenArchive::open(File::open(&sparse).unwrap(), job).unwrap();
        frozen
            .with_stored(&rel("media/a.bin"), |range| {
                assert!(range.extent().offset > 1 << 32);
                let mut bytes = [0u8; 3];
                assert_eq!(range.read_at(0, &mut bytes)?, 2);
                assert_eq!(&bytes[..2], b"hi");
                Ok(())
            })
            .unwrap();
        drop(frozen);
        assert!(
            job.read_bytes < 1_000_000,
            "the sparse hole was not streamed"
        );
    });
    drop(held);
    drop(zip);
    fs::remove_dir_all(root).unwrap();
}

/// Exhausted examination quota refuses before a fresh metadata pass or user callback.
#[test]
fn live_range_examination_is_admitted_before_native_access() {
    let root = dir("held-file-count");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let dest = root.join("plain.zip");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let mut request = base_capture(&private, &dest);
    let source = root.join("source.bin");
    fs::write(&source, b"quota source").unwrap();
    let mut file = File::open(source).unwrap();
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("m".into()),
        source_id: Some("s".into()),
        expected: None,
        reader: &mut file,
    }];
    drop(run(&limits, &cancel, |job| {
        capture(&mut request, &mut inputs, job).unwrap()
    }));
    let examinations = *run(&limits, &cancel, |job| {
        drop(FrozenArchive::open(File::open(&dest).unwrap(), job).unwrap());
        job.files
    });
    let mut closed = limits;
    closed.budget.max_files = examinations;
    let held = run(&closed, &cancel, |job| {
        let mut frozen = FrozenArchive::open(File::open(&dest).unwrap(), job).unwrap();
        assert_eq!(
            frozen.with_stored(&rel("media/a.bin"), |_| panic!("exhausted callback")),
            Err::<(), _>(Error::Quota)
        );
        drop(frozen);
        assert_eq!(job.files, examinations);
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}
