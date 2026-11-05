//! Optional generation identity controls over actual owned ZIP files, without claiming kernel search.
use super::super::*;
use knowledge_package::{
    FrozenArchive, IndexDecision, IndexOmission, IndexRequirements, VectorSpace,
};
use serde_json::{Value, json};

/// Consumer requirements are native test inputs, never read from the package's compatible flag.
fn native() -> IndexRequirements<'static> {
    IndexRequirements {
        kernel_build: "build-a",
        lexical_format: "tantivy-a",
        segment_format: "exact-a",
        processor: "processor-a",
        dictionary: "dictionary-a",
        generation: "generation-a",
        model_space: Some(VectorSpace {
            provider: "provider-a",
            model: "model-a",
            revision: "weights-a",
            dimension: 3,
            metric: "cosine",
            normalization: "unit",
            preprocessing: "image-a",
        }),
    }
}

/// Independent identity schema fixture binds every required native identity and vector-space fact.
fn identity() -> Value {
    json!({"version": 1, "kernel_build": "build-a", "lexical_format": "tantivy-a", "segment_format": "exact-a", "processor": "processor-a", "dictionary": "dictionary-a", "generation": "generation-a", "model_space": {"provider": "provider-a", "model": "model-a", "revision": "weights-a", "dimension": 3, "metric": "cosine", "normalization": "unit", "preprocessing": "image-a", "representation": "exact-f32"}})
}

/// Capture real native files, then change optional manifest metadata without altering byte inventory.
fn fixture(root: &Path, job: &mut Work<'_>, identity: Value, paths: Value) -> PathBuf {
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let page = root.join("page.md");
    fs::write(&page, b"# usable core\r\n").unwrap();
    let index = root.join("component.bin");
    fs::write(&index, [0u8, 1, 2, 3]).unwrap();
    let mut page_file = File::open(page).unwrap();
    let mut index_file = File::open(index).unwrap();
    let dest = root.join("captured.zip");
    let mut request = base_capture(&private, &dest);
    let mut inputs = [
        Input {
            path: rel("pages/a.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut page_file,
        },
        Input {
            path: rel("indexes/a.bin"),
            role: Role::Index,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut index_file,
        },
    ];
    capture(&mut request, &mut inputs, job).unwrap();
    let mut zip = zip::ZipArchive::new(File::open(dest).unwrap()).unwrap();
    let output = root.join("generation.zip");
    let mut writer = zip::ZipWriter::new(File::create(&output).unwrap());
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if member.name() == "manifest.json" {
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value["indexes"] = json!({"compatible": true, "paths": paths, "identity": identity, "extension": {"preserved": true}});
            value["coverage"]["indexes"] = json!("complete");
            bytes = serde_json_canonicalizer::to_vec(&value).unwrap();
        }
        writer
            .start_file(
                member.name(),
                zip::write::SimpleFileOptions::default().compression_method(member.compression()),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
    output
}

/// Every independent identity mismatch omits only optional indexes, not verified core bytes.
#[test]
fn complete_identity_comparison_ignores_compatible_boolean_as_authority() {
    let root = dir("index-identities");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    let cases = [
        ("kernel_build", IndexOmission::Build),
        ("lexical_format", IndexOmission::Lexical),
        ("segment_format", IndexOmission::Segment),
        ("processor", IndexOmission::Processor),
        ("dictionary", IndexOmission::Dictionary),
        ("generation", IndexOmission::Generation),
    ];
    for (key, reason) in cases {
        let own = root.join(key);
        fs::create_dir(&own).unwrap();
        let held = run(&limits, &cancel, |job| {
            let mut facts = identity();
            facts[key] = json!("incompatible");
            let path = fixture(&own, job, facts, json!(["indexes/a.bin"]));
            let frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
            assert_eq!(
                frozen.index_decision(&native()).unwrap(),
                IndexDecision::Omitted(reason)
            );
            assert!(
                frozen
                    .manifest()
                    .entries
                    .iter()
                    .any(|e| e.path.as_str() == "pages/a.md" && e.size_bytes == 15)
            );
            assert_eq!(
                frozen.manifest().indexes.as_ref().unwrap().metadata["extension"]["preserved"],
                true
            );
        });
        drop(held);
    }
    fs::remove_dir_all(root).unwrap();
}

/// Equal dimensions cannot admit a different model/provider/weights or preprocessing space.
#[test]
fn vector_model_space_and_component_closure_are_strict() {
    let root = dir("model-identities");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    for key in [
        "provider",
        "model",
        "revision",
        "metric",
        "normalization",
        "preprocessing",
    ] {
        let own = root.join(key);
        fs::create_dir(&own).unwrap();
        let held = run(&limits, &cancel, |job| {
            let mut facts = identity();
            facts["model_space"][key] = json!("different");
            let path = fixture(&own, job, facts, json!(["indexes/a.bin"]));
            let frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
            assert_eq!(
                frozen.index_paths(&native()),
                Err(Error::IndexOmitted(IndexOmission::ModelSpace))
            );
        });
        drop(held);
    }
    let own = root.join("components");
    fs::create_dir(&own).unwrap();
    let held = run(&limits, &cancel, |job| {
        let path = fixture(&own, job, identity(), json!(["pages/a.md"]));
        let frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
        assert_eq!(
            frozen.index_paths(&native()),
            Err(Error::IndexOmitted(IndexOmission::Components))
        );
    });
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

/// Metadata-only compatibility uses the same live cancellation/deadline as native byte access.
#[test]
fn compatibility_control_is_checked_before_any_metadata_work() {
    let root = dir("compatibility-control");
    let limits = bounds(8_000_000);
    for mode in ["cancel", "deadline"] {
        let own = root.join(mode);
        fs::create_dir(&own).unwrap();
        let cancel = AtomicBool::new(false);
        let held = run(&limits, &cancel, |job| {
            let path = fixture(&own, job, identity(), json!(["indexes/a.bin"]));
            if mode == "deadline" {
                job.deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
            }
            let mut frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
            let error = if mode == "cancel" {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                Error::Cancelled
            } else {
                std::thread::sleep(std::time::Duration::from_millis(110));
                Error::Deadline
            };
            assert_eq!(frozen.index_decision(&native()), Err(error));
            assert_eq!(frozen.index_paths(&native()), Err(error));
            assert_eq!(
                frozen.with_indexes(&native(), |_| panic!("expired callback")),
                Err::<(), _>(error)
            );
        });
        drop(held);
    }
    fs::remove_dir_all(root).unwrap();
}

/// Two real STORED components accept arbitrary path order; duplicates cannot substitute for coverage.
#[test]
fn component_membership_accepts_reverse_order_and_refuses_duplicate_substitution() {
    let root = dir("compatibility-order");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    for duplicate in [false, true] {
        let own = root.join(if duplicate { "duplicate" } else { "reverse" });
        fs::create_dir(&own).unwrap();
        let held = run(&limits, &cancel, |job| {
            let paths = if duplicate {
                json!(["indexes/b.bin", "indexes/b.bin"])
            } else {
                json!(["indexes/b.bin", "indexes/a.bin"])
            };
            let path = fixture(&own, job, identity(), paths);
            let mut input = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
            let output = own.join("two.zip");
            let mut writer = zip::ZipWriter::new(File::create(&output).unwrap());
            for i in 0..input.len() {
                let mut entry = input.by_index(i).unwrap();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                if entry.name() == "manifest.json" {
                    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
                    let mut fact = v["entries"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|v| v["path"] == "indexes/a.bin")
                        .unwrap()
                        .clone();
                    fact["path"] = json!("indexes/b.bin");
                    let entries = v["entries"].as_array_mut().unwrap();
                    entries.push(fact);
                    // Native manifest entries already require sorted order; optional index.paths does not.
                    entries.sort_by(|a, b| {
                        a["path"].as_str().unwrap().cmp(b["path"].as_str().unwrap())
                    });
                    bytes = serde_json_canonicalizer::to_vec(&v).unwrap();
                }
                writer
                    .start_file(
                        entry.name(),
                        zip::write::SimpleFileOptions::default()
                            .compression_method(entry.compression()),
                    )
                    .unwrap();
                writer.write_all(&bytes).unwrap();
            }
            writer
                .start_file(
                    "indexes/b.bin",
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(&[0u8, 1, 2, 3]).unwrap();
            writer.finish().unwrap();
            let mut frozen = FrozenArchive::open(File::open(&output).unwrap(), job).unwrap();
            if duplicate {
                assert_eq!(
                    frozen.index_paths(&native()),
                    Err(Error::IndexOmitted(IndexOmission::Components))
                );
            } else {
                assert_eq!(
                    frozen.index_paths(&native()).unwrap(),
                    &[rel("indexes/b.bin"), rel("indexes/a.bin")]
                );
                frozen
                    .with_indexes(&native(), |directory| {
                        let mut bytes = [0u8; 4];
                        assert_eq!(directory.read_at(&rel("indexes/b.bin"), 0, &mut bytes)?, 4);
                        assert_eq!(bytes, [0, 1, 2, 3]);
                        Ok(())
                    })
                    .unwrap();
            }
        });
        drop(held);
    }
    fs::remove_dir_all(root).unwrap();
}

/// Exact matching native facts yield candidate paths; missing metadata is an explicit omission.
#[test]
fn matching_and_missing_identity_have_distinct_results() {
    let root = dir("matching-identities");
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    for (name, value, expected) in [
        ("matching", identity(), IndexDecision::Compatible),
        (
            "missing",
            Value::Null,
            IndexDecision::Omitted(IndexOmission::MissingIdentity),
        ),
    ] {
        let own = root.join(name);
        fs::create_dir(&own).unwrap();
        let held = run(&limits, &cancel, |job| {
            let path = fixture(&own, job, value, json!(["indexes/a.bin"]));
            let mut frozen = FrozenArchive::open(File::open(path).unwrap(), job).unwrap();
            assert_eq!(frozen.index_decision(&native()).unwrap(), expected);
            if expected == IndexDecision::Compatible {
                assert_eq!(
                    frozen.index_paths(&native()).unwrap(),
                    &[rel("indexes/a.bin")]
                );
                let snapshot = frozen.snapshot_id().clone();
                frozen
                    .with_indexes(&native(), |directory| {
                        assert_eq!(directory.snapshot_id(), &snapshot);
                        assert_eq!(directory.paths(), &[rel("indexes/a.bin")]);
                        let mut bytes = [0u8; 32];
                        assert_eq!(directory.read_at(&rel("indexes/a.bin"), 1, &mut bytes)?, 3);
                        assert_eq!(&bytes[..3], &[1, 2, 3]);
                        assert_eq!(
                            directory.read_at(&rel("pages/a.md"), 0, &mut bytes),
                            Err(Error::Invalid)
                        );
                        Ok(())
                    })
                    .unwrap();
            } else {
                assert_eq!(
                    frozen.with_indexes(&native(), |_| panic!("incompatible callback")),
                    Err::<(), _>(Error::IndexOmitted(IndexOmission::MissingIdentity))
                );
            }
        });
        drop(held);
    }
    fs::remove_dir_all(root).unwrap();
}
