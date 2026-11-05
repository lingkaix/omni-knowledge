//! Shared controlled archive fixture; separate binaries each initialize ONE real account.
#![allow(dead_code, unused_imports)]
pub(super) use knowledge_core::admission::{Account, Configuration, Decision, Origin, Scheduling};
pub(super) use knowledge_core::backend::Budget;
pub(super) use knowledge_core::data::ContentDigest;
pub(super) use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
pub(super) use knowledge_package::{Capture, Input, Preset, Role, Window};
pub(super) use knowledge_retrieval::{ArchiveBounds as Bounds, ArchiveQuotas as Quotas};
pub(super) use knowledge_retrieval::{
    ArchiveSource, Content, Control, Document, Error, KernelAdapter, Locator, Mode, Query,
    ScopeView, SourceKey, StoredGeneration, StoredIdentity, StoredPlan, Work,
};
pub(super) use omni_service::authority::{LocalPolicy, NativeAuthority, Permit, TransportFacts};
pub(super) use omni_service::runtime::Runtime;
pub(super) use omni_service::{Command, ExposureMode, Id, RelativePath, Transport};
pub(super) use sha2::{Digest, Sha256};
pub(super) use std::collections::BTreeMap;
pub(super) use std::fs::{self, File};
pub(super) use std::io::{Read, Write};
pub(super) use std::os::unix::fs::{MetadataExt, PermissionsExt};
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::sync::{Arc, atomic::AtomicBool};
pub(super) use std::time::{Duration, Instant};

/// Build inputs bind actual source and consuming machinery; a digest is not read permission.
pub(super) const BUILD: &str = concat!(env!("CARGO_PKG_NAME"), ":stored-generation-contract-v1");
/// The exact archived canonical source name is also the existing kernel writer's source name.
pub(super) const SOURCE: &str = "indexes/native/source.txt";
/// Deterministic real source with distinct English, Chinese and Japanese lexical/scan evidence.
pub(super) const TEXT: &[u8] =
    "alpha shared archive\n机器学习检索\n日本語の検索エンジン\n".as_bytes();
/// Hash controlled fixture bytes, not an authority label.
pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Use the actual process account, never a numeric stand-in grant.
pub(super) fn reserve(need: Reservation) -> Work {
    match Work::reserve(need, Origin::BeforeVerification).unwrap() {
        Decision::Ready(work) => work,
        Decision::Waiting(reason) => panic!("fixture unexpectedly waiting: {reason:?}"),
    }
}
/// Keep original coordinates and source identity across filesystem and archived generations.
pub(super) fn view() -> ScopeView {
    ScopeView {
        scope: "archive-shard".into(),
        unavailable_objects: 0,
        documents: vec![Document {
            key: SourceKey {
                namespace: "archive-contract".into(),
                id: "first-source".into(),
            },
            revision: digest(TEXT),
            digest: digest(TEXT),
            size: TEXT.len() as u64,
            name: "source.txt".into(),
            title: "Canonical archive".into(),
            media_type: "text/plain".into(),
            content: Content::Text {
                path: SOURCE.into(),
                range: None,
            },
            text_ranges: None,
            language: Some("unicode".into()),
            metadata: BTreeMap::new(),
            locator: Some("pages/source".into()),
        }],
    }
}
/// Full finite M5 verification ceilings, not an archive-size or corpus cap increase.
pub(super) fn bounds() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 100_000,
            max_bytes: 128 * 1024 * 1024,
            max_millis: 60_000,
            max_memory_bytes: 1024 * 1024 * 1024,
        },
        quotas: Quotas {
            max_archive_bytes: 32 * 1024 * 1024,
            max_entries: 512,
            max_entry_expanded: 8 * 1024 * 1024,
            max_total_expanded: 32 * 1024 * 1024,
            max_ratio: 1000,
            max_path_bytes: 1024,
            max_depth: 16,
        },
    }
}
/// Gather only actual writer-created native components; this is bounded fixture setup, not ingestion.
pub(super) fn components(directory: &Path, relative: &Path, output: &mut Vec<(String, File)>) {
    for entry in fs::read_dir(directory.join(relative)).unwrap() {
        let entry = entry.unwrap();
        let child = relative.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            components(directory, &child, output);
        } else {
            output.push((
                format!("indexes/native/{}", child.to_str().unwrap()),
                File::open(entry.path()).unwrap(),
            ));
        }
    }
}
/// Public M5 capture produces the inventory; the external package producer adds its actual native tuple.
/// M5 capture itself still omits indexes. Native optional identity is external wire data, never a grant.
pub(super) fn archive_fixture(
    root: &Path,
    native_dir: &Path,
    source: &Path,
    generation: &str,
    account: &Account,
) -> PathBuf {
    archive_fixture_with_space(root, native_dir, source, generation, account, None)
}
pub(super) fn archive_fixture_with_space(
    root: &Path,
    native_dir: &Path,
    source: &Path,
    generation: &str,
    account: &Account,
    space: Option<&knowledge_search::ModelIdentity>,
) -> PathBuf {
    let mut files = Vec::new();
    components(native_dir, Path::new(""), &mut files);
    files.push((SOURCE.into(), File::open(source).unwrap()));
    let paths: Vec<_> = files.iter().map(|(name, _)| name.clone()).collect();
    let output = root.join("captured.zip");
    let private = root.join("capture-private");
    fs::create_dir(&private).unwrap();
    let limits = bounds();
    let cancel = AtomicBool::new(false);
    let mut held = match account
        .reserve(
            Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        Decision::Waiting(r) => panic!("fixture waiting {r:?}"),
    };
    let mut job = knowledge_package::Work::new(&limits, &mut held, &cancel).unwrap();
    let mut inputs: Vec<_> = files
        .iter_mut()
        .map(|(name, file)| Input {
            path: knowledge_core::data::RelativePath::new(name.as_str()).unwrap(),
            role: Role::Index,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: file,
        })
        .collect();
    let mut request = Capture {
        operation_id: "native-archive",
        preset: Preset::Frozen,
        library_id: "archive-library",
        name: "Native archive",
        text_root: "pages",
        window: Window {
            start: "2026-11-05T11:00:00Z".into(),
            end: "2026-11-05T11:00:01Z".into(),
        },
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: true,
        required_features: vec![],
        private_dir: &private,
        destination: &output,
    };
    let captured = knowledge_package::capture(&mut request, &mut inputs, &mut job).unwrap();
    drop(captured);
    drop(inputs);
    drop(files);
    drop(job);
    held.finish().unwrap();
    let native = StoredIdentity::new(BUILD).unwrap();
    let requirements = native.requirements(generation);
    let mut zip = zip::ZipArchive::new(File::open(output).unwrap()).unwrap();
    let final_path = root.join("frozen.zip");
    let mut writer = zip::ZipWriter::new(File::create(&final_path).unwrap());
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if member.name() == "manifest.json" {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            value["indexes"] = serde_json::json!({"compatible": true, "paths": paths, "identity": {
                "version": 1, "kernel_build": requirements.kernel_build, "lexical_format": requirements.lexical_format,
                "segment_format": requirements.segment_format, "processor": requirements.processor, "dictionary": requirements.dictionary,
                "generation": generation, "model_space": space.map(|s|serde_json::json!({
                    "provider":s.provider,"model":s.model,"revision":s.revision,"dimension":s.dimension,
                    "metric":"cosine","normalization":"raw","preprocessing":s.preprocessing,"representation":"exact-f32" })) }});
            value["coverage"]["indexes"] = serde_json::json!("complete");
            bytes = serde_json::to_vec(&value).unwrap();
        }
        writer
            .start_file(
                member.name(),
                zip::write::SimpleFileOptions::default().compression_method(member.compression()),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().sync_all().unwrap();
    final_path
}
/// Actual service owner-selected FD and current policy/version carrier used by the archive adapter.
pub(super) struct SelectedArchive {
    pub(super) file: File,
    pub(super) permit: Permit,
    pub(super) library: Id,
    pub(super) path: RelativePath,
}
impl ArchiveSource for SelectedArchive {
    /// Check the real service permit and descriptor-relative current source, not a selected constant.
    fn check(&self) -> Result<(), Error> {
        self.permit
            .check_path(&self.library, &self.path)
            .map_err(|_| Error::Inventory)?;
        self.permit
            .with_root(&self.library, |root| {
                let fd = rustix::fs::openat(
                    root,
                    self.path.as_str(),
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| omni_service::ErrorCode::Changed)?;
                let now = File::from(fd)
                    .metadata()
                    .map_err(|_| omni_service::ErrorCode::Changed)?;
                let held = self
                    .file
                    .metadata()
                    .map_err(|_| omni_service::ErrorCode::Changed)?;
                if (
                    now.dev(),
                    now.ino(),
                    now.len(),
                    now.mtime(),
                    now.mtime_nsec(),
                    now.ctime(),
                    now.ctime_nsec(),
                ) != (
                    held.dev(),
                    held.ino(),
                    held.len(),
                    held.mtime(),
                    held.mtime_nsec(),
                    held.ctime(),
                    held.ctime_nsec(),
                ) {
                    return Err(omni_service::ErrorCode::Changed);
                }
                Ok(())
            })
            .map_err(|_| Error::Inventory)
    }
    /// Return only the original held native descriptor after the current owner check.
    fn file(&self) -> Result<&File, Error> {
        self.check()?;
        Ok(&self.file)
    }
}
/// Bind the actual running Claim and its complete observed key to every controlled native read.
pub(super) fn claim_control(
    source: Arc<SelectedArchive>,
    claim: Arc<knowledge_core::jobs::Claim>,
    key: Key<'_>,
) -> Control {
    let context = key.context.to_owned();
    let library = key.library.to_owned();
    let mount = key.mount.map(str::to_owned);
    let source_id = key.source.to_owned();
    let version = key.version.clone();
    let recipe = key.recipe.clone();
    let processor = key.processor.clone();
    Control::guarded(
        claim.deadline(),
        Arc::new(move || {
            source.check().is_ok()
                && claim
                    .stop_requested(Key {
                        context: &context,
                        library: &library,
                        mount: mount.as_deref(),
                        source: &source_id,
                        version: &version,
                        recipe: &recipe,
                        processor: &processor,
                    })
                    .is_ok_and(|stop| stop.is_none())
        }),
    )
}

/// Search through the real archive-backed Directory and mature shared query implementation.
pub(super) fn query(
    generation: StoredGeneration,
    mode: Mode,
    text: &str,
    control: &Control,
) -> knowledge_retrieval::StoredResult {
    generation
        .search(
            Query {
                mode,
                query: text.into(),
                scopes: vec![view()],
                limit: 10,
            },
            control,
        )
        .unwrap()
}
