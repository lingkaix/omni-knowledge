//! Native-caller compatibility checks; an archive's compatible boolean is only a claim.
use crate::{
    ContentDigest, Error, FrozenArchive, Manifest, Method, RelativePath, Role, StoredRange, Work,
    gate, preallocate,
};
use serde_json::Value;

/// Exact identities supplied by the native kernel that will consume a generation.
/// No package field supplies the caller's requirements or permission to execute a processor.
#[derive(Clone, Copy, Debug)]
pub struct IndexRequirements<'a> {
    /// Native kernel build identity.
    pub kernel_build: &'a str,
    /// Native lexical index format identity.
    pub lexical_format: &'a str,
    /// Exact disk-backed float32 segment format identity.
    pub segment_format: &'a str,
    /// Processor/build fingerprint of the indexed representation.
    pub processor: &'a str,
    /// Token dictionary/analyzer fingerprint.
    pub dictionary: &'a str,
    /// Immutable generation identity admitted by the caller.
    pub generation: &'a str,
    /// Complete model-space identity, or None for a lexical-only generation.
    pub model_space: Option<VectorSpace<'a>>,
}

/// Complete exact-vector space identity; equal dimension alone is not compatibility.
#[derive(Clone, Copy, Debug)]
pub struct VectorSpace<'a> {
    /// Embedding provider identity.
    pub provider: &'a str,
    /// Model name.
    pub model: &'a str,
    /// Exact weights/revision fingerprint.
    pub revision: &'a str,
    /// Float32 vector dimension.
    pub dimension: u32,
    /// Native distance metric.
    pub metric: &'a str,
    /// Vector normalization identity.
    pub normalization: &'a str,
    /// Input preprocessing fingerprint.
    pub preprocessing: &'a str,
}

/// Explicit reason optional indexes are not usable; core bytes remain readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexOmission {
    /// No optional generation is inventoried.
    Absent,
    /// The producer did not claim a complete generation.
    ProducerOmitted,
    /// Unknown required package semantics cannot be used by the caller.
    RequiredFeature,
    /// Missing, duplicated, non-index, non-STORED, or incomplete component paths.
    Components,
    /// Full native identity metadata is absent or malformed.
    MissingIdentity,
    /// Kernel build differs.
    Build,
    /// Lexical format differs.
    Lexical,
    /// Exact-vector segment format differs.
    Segment,
    /// Processor fingerprint differs.
    Processor,
    /// Dictionary/analyzer fingerprint differs.
    Dictionary,
    /// Immutable generation differs.
    Generation,
    /// Model/provider/revision/dimension/metric/normalization/preprocessing differs.
    ModelSpace,
}

/// Result of comparing verified package facts to the actual consumer's identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexDecision {
    /// All identities and the complete STORED component inventory match.
    Compatible,
    /// Optional generation omitted for the given reason, without changing core usability.
    Omitted(IndexOmission),
}

/// Read-only component directory over one compatible, verified, pinned archive generation.
/// This is byte IO, not a synthetic filesystem, extracted index, or kernel search result.
/// Component callbacks cannot escape the source/version/admission lifetime.
pub struct IndexDirectory<'r, 'w, 'a> {
    archive: &'r mut FrozenArchive<'w, 'a>,
}

impl IndexDirectory<'_, '_, '_> {
    /// Exact component names from the verified complete index inventory.
    pub fn paths(&self) -> &[RelativePath] {
        &self
            .archive
            .manifest()
            .indexes
            .as_ref()
            .expect("admitted directory")
            .paths
    }

    /// Pinned canonical snapshot identity, not an access grant.
    pub fn snapshot_id(&self) -> &ContentDigest {
        self.archive.snapshot_id()
    }

    /// Exact held native version for the generation provider's cache/pin identity.
    pub fn version(&self) -> crate::ArchiveVersion {
        self.archive.version()
    }

    /// Open only a member of this complete component set; no external path is consulted.
    pub fn with_component<T>(
        &mut self,
        path: &RelativePath,
        body: impl FnOnce(&mut StoredRange<'_, '_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        gate(self.archive.control())?;
        let mut found = false;
        for candidate in self.paths() {
            gate(self.archive.control())?;
            if candidate == path {
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::Invalid);
        }
        self.archive.with_stored(path, body)
    }

    /// Bounded entry-relative positional read for the native generation adapter.
    pub fn read_at(
        &mut self,
        path: &RelativePath,
        offset: u64,
        output: &mut [u8],
    ) -> Result<usize, Error> {
        self.with_component(path, |entry| entry.read_at(offset, output))
    }
}

impl FrozenArchive<'_, '_> {
    /// Lend a concrete read-only component directory only after exact native compatibility.
    /// Optional incompatibility is typed and leaves core bytes untouched and usable.
    /// Revalidate again after the entire kernel adapter callback before returning its result.
    pub fn with_indexes<T>(
        &mut self,
        native: &IndexRequirements<'_>,
        body: impl FnOnce(&mut IndexDirectory<'_, '_, '_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if let IndexDecision::Omitted(reason) = self.index_decision(native)? {
            return Err(Error::IndexOmitted(reason));
        }
        self.revalidate()?;
        let result = body(&mut IndexDirectory { archive: self });
        self.revalidate()?;
        result
    }

    /// Check complete native identities and component inventory without trusting DTO booleans.
    /// This does not open a kernel index, rebuild it, extract it, or activate external sources.
    /// Cancellation/deadline or admission failure is an Error, never an optional-index omission.
    pub fn index_decision(&self, native: &IndexRequirements<'_>) -> Result<IndexDecision, Error> {
        decision(self.manifest(), native, self.control())
    }

    /// Exact inventoried paths for a compatible generation, otherwise an explicit omission.
    /// The returned names borrow the reader pin and cannot outlive its admission.
    pub fn index_paths(&self, native: &IndexRequirements<'_>) -> Result<&[RelativePath], Error> {
        match self.index_decision(native)? {
            IndexDecision::Compatible => Ok(&self
                .manifest()
                .indexes
                .as_ref()
                .expect("checked index")
                .paths),
            IndexDecision::Omitted(reason) => Err(Error::IndexOmitted(reason)),
        }
    }
}

/// Compare facts using the existing Work and pre-admitted, order-independent component membership.
fn decision(
    manifest: &Manifest,
    native: &IndexRequirements<'_>,
    work: &Work<'_>,
) -> Result<IndexDecision, Error> {
    use IndexOmission::*;
    gate(work)?;
    let omit = |reason| Ok(IndexDecision::Omitted(reason));
    let Some(index) = &manifest.indexes else {
        return omit(Absent);
    };
    if !manifest.required_features.is_empty() {
        return omit(RequiredFeature);
    }
    if !index.compatible || manifest.coverage.indexes != crate::IndexCoverage::Complete {
        return omit(ProducerOmitted);
    }
    if !components(manifest, &index.paths, work)? {
        return omit(Components);
    }
    let Some(Value::Object(identity)) = index.metadata.get("identity") else {
        return omit(MissingIdentity);
    };
    // Version 1 is the complete identity schema, not the ZIP or mutable-home version.
    if identity.get("version").and_then(Value::as_u64) != Some(1) {
        return omit(MissingIdentity);
    }
    for (key, expected, reason) in [
        ("kernel_build", native.kernel_build, Build),
        ("lexical_format", native.lexical_format, Lexical),
        ("segment_format", native.segment_format, Segment),
        ("processor", native.processor, Processor),
        ("dictionary", native.dictionary, Dictionary),
        ("generation", native.generation, Generation),
    ] {
        match identity.get(key).and_then(Value::as_str) {
            Some(value) if !value.is_empty() && !expected.is_empty() => {
                if value != expected {
                    return omit(reason);
                }
            }
            _ => return omit(MissingIdentity),
        }
    }
    match (identity.get("model_space"), native.model_space) {
        (Some(Value::Null), None) => {}
        (Some(Value::Object(model)), Some(expected)) => {
            if expected.dimension == 0
                || model.get("dimension").and_then(Value::as_u64)
                    != Some(u64::from(expected.dimension))
            {
                return omit(ModelSpace);
            }
            for (key, value) in [
                ("provider", expected.provider),
                ("model", expected.model),
                ("revision", expected.revision),
                ("metric", expected.metric),
                ("normalization", expected.normalization),
                ("preprocessing", expected.preprocessing),
            ] {
                if value.is_empty() || model.get(key).and_then(Value::as_str) != Some(value) {
                    return omit(ModelSpace);
                }
            }
            if model.len() != 8 {
                return omit(ModelSpace);
            }
            if model.get("representation").and_then(Value::as_str) != Some("exact-f32") {
                return omit(Segment);
            }
        }
        _ => return omit(ModelSpace),
    }
    if identity.len() != 8 {
        return omit(MissingIdentity);
    }
    gate(work)?;
    Ok(IndexDecision::Compatible)
}

/// Insert/remove each path once in a bounded B-tree: O((entries+components) log entries), any order.
/// Scratch survives all nodes and is funded before the first tree allocation; no corpus cap is added.
fn components(manifest: &Manifest, paths: &[RelativePath], work: &Work<'_>) -> Result<bool, Error> {
    gate(work)?;
    if paths.is_empty() {
        return Ok(false);
    }
    let count = manifest
        .entries
        .len()
        .checked_add(paths.len())
        .ok_or(Error::TooLarge)?;
    let _pin = preallocate(
        work,
        (count as u64)
            .checked_mul(128)
            .and_then(|n| n.checked_add(4096))
            .ok_or(Error::TooLarge)?,
    )?;
    let mut remaining = std::collections::BTreeSet::new();
    for entry in &manifest.entries {
        gate(work)?;
        if entry.role == Role::Index
            && (entry.method != Method::Stored || !remaining.insert(entry.path.as_str()))
        {
            return Ok(false);
        }
    }
    for path in paths {
        gate(work)?;
        if !remaining.remove(path.as_str()) {
            return Ok(false);
        }
    }
    gate(work)?;
    Ok(remaining.is_empty())
}
