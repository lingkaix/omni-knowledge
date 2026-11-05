//! Native OWNER-selected actual model artifact/metadata/receiver context, not DTO authority.
use super::source::refused_error;
use super::*;
use crate::embedding::EmbeddingConfig;
use knowledge_search::ModelIdentity;
use omni_service::authority::SelectionLease;
use std::{
    os::unix::fs::{FileExt, MetadataExt},
    sync::Arc,
};

/// Opaque selected model package/metadata descriptors and explicit native owner's receiver choice.
/// Clone shares the SAME actual pins/credit; it grants no source or destination/write permission.
#[derive(Clone)]
pub struct NativeEmbeddingSelection(Arc<ModelOwner>);
struct Inputs {
    package: File,
    manifest: File,
    config: EmbeddingConfig,
    work: Work,
}
struct ModelOwner {
    model: ModelIdentity,
    config: EmbeddingConfig,
    package: File,
    manifest: File,
    package_stamp: [u64; 8],
    manifest_stamp: [u64; 8],
    lease: SelectionLease,
    _work: Work,
}
impl NativeEmbeddingSelection {
    /// Complete metadata/hash-window/retained descriptors/configuration envelope before inspection.
    /// Model size is streamed; this is not a model/corpus-byte ceiling or model weight allocation.
    pub fn requirement(config: &EmbeddingConfig) -> Result<Reservation> {
        let bytes = [
            &config.endpoint,
            &config.model,
            &config.package_fingerprint,
            &config.embedding_space,
        ]
        .into_iter()
        .try_fold(0u64, |n, s| n.checked_add(s.len() as u64))
        .context("model configuration size overflow")?;
        Ok(Reservation {
            memory_bytes: bytes
                .checked_mul(8)
                .and_then(|n| n.checked_add(1024 * 1024))
                .context("model selection plan overflow")?,
            temporary_bytes: 0,
        })
    }
    /// Read-only selected facts after actual descriptor/configuration checks, not a portable grant.
    pub fn model(&self) -> Result<&ModelIdentity> {
        self.check()?;
        Ok(&self.0.model)
    }
    /// Owner-selected receiver/configuration DATA; loopback by itself never authorized this choice.
    pub fn config(&self) -> Result<&EmbeddingConfig> {
        self.check()?;
        Ok(&self.0.config)
    }
    pub(super) fn check(&self) -> Result<()> {
        self.0.lease.check().map_err(refused_error)?;
        anyhow::ensure!(
            stamp(&self.0.package)? == self.0.package_stamp
                && stamp(&self.0.manifest)? == self.0.manifest_stamp,
            "actual selected model package/metadata changed"
        );
        Ok(())
    }
    pub(super) fn check_source(&self, source: &NativeSource) -> Result<()> {
        self.check()?;
        source.check_model_selection(&self.0.lease)
    }
}
impl NativeLibraryService {
    /// Select an actual immutable package FD and complete ModelIdentity JSON FD with an explicit
    /// OWNER receiver choice. No transport/descriptor data can mint Owner or substitute library IDs.
    /// Incoming Work is already reserved/split from the ONE account BEFORE any native inspection.
    /// No implicit model download/provider start and no hash-FD-then-reopen occurs.
    pub fn select_embedding(
        &self,
        owner: &Owner,
        package: File,
        manifest: File,
        config: EmbeddingConfig,
        budget: knowledge_core::backend::Budget,
        work: Work,
    ) -> Result<NativeEmbeddingSelection> {
        let inputs = Inputs {
            package,
            manifest,
            config,
            work,
        };
        let need = NativeEmbeddingSelection::requirement(&inputs.config)?;
        anyhow::ensure!(
            inputs.work.remaining()?.memory_bytes >= need.memory_bytes,
            "native model selection is not fully prepaid"
        );
        anyhow::ensure!(
            budget.max_files >= 2
                && budget.max_millis > 0
                && need.memory_bytes <= budget.max_memory_bytes,
            "native model selection budget exhausted"
        );
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(budget.max_millis))
            .context("model selection deadline overflow")?;
        let lease = owner
            .pin_selection(&self.selection)
            .map_err(refused_error)?;
        for file in [&inputs.package, &inputs.manifest] {
            let flags = rustix::fs::fcntl_getfl(file)?;
            anyhow::ensure!(
                !flags.intersects(rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::RDWR),
                "native model selection requires actual read-only descriptors"
            );
            anyhow::ensure!(
                file.metadata()?.is_file(),
                "native model selection requires regular artifacts"
            );
        }
        let package_stamp = stamp(&inputs.package)?;
        let manifest_stamp = stamp(&inputs.manifest)?;
        anyhow::ensure!(
            package_stamp[2] > 0
                && manifest_stamp[2] <= 65536
                && package_stamp[2]
                    .checked_add(manifest_stamp[2])
                    .is_some_and(|n| n <= budget.max_bytes),
            "native model selection byte budget exhausted"
        );
        crate::embedding::validate_config(&inputs.config)?;
        let mut bytes = vec![0; manifest_stamp[2] as usize + 1];
        let mut offset = 0;
        while offset < bytes.len() {
            checked(&lease, &inputs.manifest, manifest_stamp, deadline)?;
            let n = match inputs.manifest.read_at(&mut bytes[offset..], offset as u64) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            checked(&lease, &inputs.manifest, manifest_stamp, deadline)?;
            if n == 0 {
                break;
            }
            offset += n;
        }
        anyhow::ensure!(
            offset as u64 == manifest_stamp[2],
            "native model metadata changed while reading"
        );
        bytes.truncate(offset);
        let model: ModelIdentity =
            serde_json::from_slice(&bytes).context("complete native model identity required")?;
        for field in [
            &model.provider,
            &model.model,
            &model.revision,
            &model.embedding_space,
            &model.preprocessing,
        ] {
            anyhow::ensure!(
                !field.trim().is_empty(),
                "native model identity is incomplete"
            );
        }
        anyhow::ensure!(
            model.model == inputs.config.model
                && model.dimension == inputs.config.dimension
                && model.embedding_space == inputs.config.embedding_space
                && model.package_fingerprint == inputs.config.package_fingerprint,
            "native selected model and receiver configuration differ"
        );
        let mut hash = Sha256::new();
        let mut at = 0u64;
        let mut window = [0u8; 65536];
        loop {
            checked(&lease, &inputs.package, package_stamp, deadline)?;
            let n = match inputs.package.read_at(&mut window, at) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            checked(&lease, &inputs.package, package_stamp, deadline)?;
            if n == 0 {
                break;
            }
            at = at
                .checked_add(n as u64)
                .context("model package offset overflow")?;
            anyhow::ensure!(at <= package_stamp[2], "native selected model package grew");
            hash.update(&window[..n]);
        }
        anyhow::ensure!(
            at == package_stamp[2] && hex::encode(hash.finalize()) == model.package_fingerprint,
            "actual selected model package fingerprint mismatch"
        );
        checked(&lease, &inputs.manifest, manifest_stamp, deadline)?;
        drop(bytes);
        let mut prepared = Arc::new(ModelOwner {
            model,
            config: inputs.config,
            package: inputs.package,
            manifest: inputs.manifest,
            package_stamp,
            manifest_stamp,
            lease,
            _work: inputs.work,
        });
        // All members/metadata and allocations exist before configuration epoch CAS. Never reset
        // policy/exclusions from a stale copy. A failed preparation does not advance the old epoch.
        let committed = owner
            .revise_selection(&prepared.lease)
            .map_err(refused_error)?;
        Arc::get_mut(&mut prepared)
            .expect("unshared prepared native model")
            .lease = committed;
        Ok(NativeEmbeddingSelection(prepared))
    }
}
fn stamp(file: &File) -> Result<[u64; 8]> {
    let m = file.metadata()?;
    Ok([
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        m.ctime() as u64,
        m.ctime_nsec() as u64,
        m.nlink(),
    ])
}
fn checked(
    lease: &SelectionLease,
    file: &File,
    expected: [u64; 8],
    deadline: Instant,
) -> Result<()> {
    lease.check().map_err(refused_error)?;
    anyhow::ensure!(
        Instant::now() < deadline,
        "native model selection deadline elapsed"
    );
    anyhow::ensure!(
        stamp(file)? == expected,
        "actual native model artifact changed"
    );
    Ok(())
}
