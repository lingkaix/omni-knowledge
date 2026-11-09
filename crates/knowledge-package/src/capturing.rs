//! Streaming capture into an exclusively created native ZIP.
use super::*;
use crate::gated::GatedWrite;
use std::cell::RefCell;
use std::io::{Seek, SeekFrom};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Stream selected inputs into a private archive and link it only if the destination is absent.
/// Metadata clones and ZIP state are admitted before construction; source bytes use one window.
/// Failed partial files are retained for inspection, never adopted or replaced on retry.
pub fn capture(
    capture: &mut Capture<'_>,
    inputs: &mut [Input<'_>],
    work: &mut Work<'_>,
) -> Result<Captured, Error> {
    capture_inner(capture, inputs, None, work)
}

/// Capture the actual selected native generation with explicit producer identity data.
/// The index set must be nonempty and all its inputs must be STORED; the consumer still verifies native formats.
/// The ordinary `capture` entrypoint retains its explicit unqualified-index omission.
pub fn capture_with_indexes(
    capture: &mut Capture<'_>,
    inputs: &mut [Input<'_>],
    native: &IndexRequirements<'_>,
    work: &mut Work<'_>,
) -> Result<Captured, Error> {
    capture_inner(capture, inputs, Some(native), work)
}

/// One encoder and ownership path for ordinary captures and native indexed captures.
fn capture_inner(
    capture: &mut Capture<'_>,
    inputs: &mut [Input<'_>],
    native: Option<&IndexRequirements<'_>>,
    work: &mut Work<'_>,
) -> Result<Captured, Error> {
    let identity_bytes = native.map_or(Ok(0), |n| {
        let fields = [
            n.kernel_build,
            n.lexical_format,
            n.segment_format,
            n.processor,
            n.dictionary,
            n.generation,
        ];
        if fields.iter().any(|f| f.is_empty()) || !inputs.iter().any(|i| i.role == Role::Index) {
            return Err(Error::Invalid);
        }
        let mut size = fields.iter().try_fold(4096u64, |sum, f| {
            sum.checked_add(f.len() as u64).ok_or(Error::TooLarge)
        })?;
        if let Some(m) = n.model_space {
            if m.dimension == 0 {
                return Err(Error::Invalid);
            }
            for field in [
                m.provider,
                m.model,
                m.revision,
                m.metric,
                m.normalization,
                m.preprocessing,
            ] {
                if field.is_empty() {
                    return Err(Error::Invalid);
                }
                size = size
                    .checked_add(field.len() as u64)
                    .ok_or(Error::TooLarge)?;
            }
        }
        size.checked_mul(16).ok_or(Error::TooLarge)
    })?;
    let _native_metadata = preallocate(work, identity_bytes)?;
    operation_id(capture.operation_id)?;
    gate(work)?;
    if inputs.len() as u64 + 1 > work.bounds.quotas.max_entries {
        return Err(Error::Quota);
    }
    // An unavoidable framing minimum is only an early refusal, never allocation permission.
    let minimum = inputs.iter().try_fold(22u64 + 76 + 26, |n, i| {
        n.checked_add(76 + 2 * i.path.as_str().len() as u64)
            .ok_or(Error::TooLarge)
    })?;
    if minimum > work.bounds.budget.max_bytes {
        return Err(Error::TooLarge);
    }
    if minimum > work.bounds.quotas.max_archive_bytes {
        return Err(Error::Quota);
    }
    metadata::capture(capture, inputs, work)?;
    let root = native::bind(capture.private_dir, work)?;
    let final_name = format!("{}.zip", capture.operation_id);
    let partial_name = format!("{}.partial", capture.operation_id);
    if native::exists(&root, &final_name, work)? || native::exists(&root, &partial_name, work)? {
        return Err(Error::Conflict);
    }
    for input in inputs.iter() {
        manifest::path(input.path.as_str())?;
        if input.path.as_str().len() > work.bounds.quotas.max_path_bytes
            || input.path.as_str().split('/').count() > work.bounds.quotas.max_depth
        {
            return Err(Error::Quota);
        }
        if input.role == Role::Original && (input.mount_id.is_none() || input.source_id.is_none()) {
            return Err(Error::Invalid);
        }
    }
    let file = native::file(&root, Path::new(&partial_name), true, work)?;
    let mut file = {
        // miniz's compressor tables/window plus one I/O window. STORED-only captures need no compressor.
        let encoder_bytes = if inputs.iter().any(|i| i.role == Role::Core) {
            1_048_576
        } else {
            0
        };
        let _scratch = preallocate(work, encoder_bytes + 8192)?;
        let cell = RefCell::new(&mut *work);
        let failure = Cell::new(None);
        let output = GatedWrite::new(file, &cell, &failure);
        let mut zip = ZipWriter::new(output);
        let result = (|| {
            let mut digested = Vec::with_capacity(inputs.len());
            let mut total = 0u64;
            for input in inputs.iter_mut() {
                note_file(&mut cell.borrow_mut())?;
                let method = method_for(input.role)?;
                zip.start_file(
                    input.path.as_str(),
                    SimpleFileOptions::default()
                        .large_file(true)
                        .compression_method(match method {
                            Method::Stored => CompressionMethod::Stored,
                            Method::Deflate => CompressionMethod::Deflated,
                        }),
                )
                .map_err(|_| failure.get().unwrap_or(Error::Io))?;
                let mut sha = Sha256::new();
                let mut size = 0u64;
                let mut buffer = [0u8; 8192];
                loop {
                    let count = {
                        let mut job = cell.borrow_mut();
                        gate(&job)?;
                        let count = input.reader.read(&mut buffer).map_err(|_| Error::Io)?;
                        size = size.checked_add(count as u64).ok_or(Error::Quota)?;
                        total = total.checked_add(count as u64).ok_or(Error::Quota)?;
                        if size > job.bounds.quotas.max_entry_expanded
                            || total > job.bounds.quotas.max_total_expanded
                        {
                            return Err(Error::Quota);
                        }
                        note_read(&mut job, count as u64)?;
                        count
                    };
                    if count == 0 {
                        break;
                    }
                    sha.update(&buffer[..count]);
                    zip.write_all(&buffer[..count])
                        .map_err(|_| failure.get().unwrap_or(Error::Io))?;
                }
                let digest =
                    ContentDigest::new(hex(&sha.finalize())).map_err(|_| Error::Invalid)?;
                if input.expected.as_ref().is_some_and(|e| e != &digest) {
                    return Err(Error::Drift);
                }
                digested.push(Digested {
                    path: input.path.clone(),
                    role: input.role,
                    method,
                    mount_id: input.mount_id.clone(),
                    source_id: input.source_id.clone(),
                    digest,
                    size,
                });
            }
            digested.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
            if digested.windows(2).any(|p| p[0].path == p[1].path) {
                return Err(Error::Invalid);
            }
            let mut coverage = coverage_for(
                digested.len() as u64,
                capture.omitted,
                &capture.external,
                digested.iter().any(|i| i.role == Role::Index),
            );
            let mut indexes = index_set(&digested);
            if let Some(n) = native {
                if digested
                    .iter()
                    .any(|i| i.role == Role::Index && i.method != Method::Stored)
                {
                    return Err(Error::Invalid);
                }
                let index = indexes.as_mut().ok_or(Error::Invalid)?;
                let model = n.model_space.map(|m| serde_json::json!({
                    "provider":m.provider,"model":m.model,"revision":m.revision,"dimension":m.dimension,
                    "metric":m.metric,"normalization":m.normalization,"preprocessing":m.preprocessing,
                    "representation":"exact-f32"
                }));
                index.metadata.insert("identity".into(), serde_json::json!({
                    "version":1,"kernel_build":n.kernel_build,"lexical_format":n.lexical_format,
                    "segment_format":n.segment_format,"processor":n.processor,"dictionary":n.dictionary,
                    "generation":n.generation,"model_space":model
                }));
                index.compatible = true;
                index.fallback = None;
                coverage.indexes = IndexCoverage::Complete;
            }
            if capture.require_complete
                && (coverage.unverified > 0 || coverage.pending > 0 || coverage.unavailable > 0)
            {
                return Err(Error::RequireComplete);
            }
            let manifest = Manifest {
                version: 1,
                library_id: capture.library_id.to_owned(),
                name: capture.name.to_owned(),
                text_root: capture.text_root.to_owned(),
                preset: capture.preset,
                required_features: capture.required_features.clone(),
                mounts: capture.mounts.clone(),
                entries: digested
                    .iter()
                    .map(|i| EntryRecord {
                        path: i.path.clone(),
                        size_bytes: i.size,
                        sha256: i.digest.clone(),
                        method: i.method,
                        role: i.role,
                        mount_id: i.mount_id.clone(),
                        source_id: i.source_id.clone(),
                    })
                    .collect(),
                external: {
                    let mut e = capture.external.clone();
                    e.sort_by(|a, b| {
                        (a.source_id.as_str(), a.path.as_str())
                            .cmp(&(b.source_id.as_str(), b.path.as_str()))
                    });
                    e
                },
                indexes,
                history: blob_for(&digested, Role::History),
                journal: blob_for(&digested, Role::Journal),
                window: capture.window.clone(),
                coverage,
                extra: Map::new(),
            };
            let canonical = manifest::emit(&manifest, &mut cell.borrow_mut())?;
            zip.start_file(
                "manifest.json",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .map_err(|_| failure.get().unwrap_or(Error::Io))?;
            zip.write_all(&canonical)
                .map_err(|_| failure.get().unwrap_or(Error::Io))?;
            Ok::<(), Error>(())
        })();
        result?;
        zip.finish()
            .map_err(|_| failure.get().unwrap_or(Error::Io))?
            .into_file()
    };
    file.sync_all().map_err(|_| Error::Io)?;
    file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;
    let verified = open_verified(&mut file, work)?;
    // Exclusive link, not rename-overwrite. Preserve any earlier generation at either name.
    note_file(work)?;
    rustix::fs::linkat(
        &root,
        partial_name.as_str(),
        &root,
        final_name.as_str(),
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|_| Error::Conflict)?;
    rustix::fs::unlinkat(&root, partial_name.as_str(), rustix::fs::AtFlags::empty())
        .map_err(|_| Error::Indeterminate)?;
    let linked = native::link(&root, &final_name, capture.destination, work)?;
    Ok(Captured {
        snapshot_id: verified.snapshot_id,
        private_path: capture.private_dir.join(final_name),
        installed: linked.then(|| capture.destination.to_path_buf()),
        linked,
    })
}
