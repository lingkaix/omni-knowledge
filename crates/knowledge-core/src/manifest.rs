//! Bounded, pure manifest inspection and name-only candidate editing.
//!
//! Storage owns the format. This module never opens a directory, initializes/migrates data,
//! activates a mount, interprets imported instructions, or grants local/service permissions.
//! Use `original_bytes` for byte-exact inspection/copy; metadata snapshots are not manifest writers.
use crate::data::{BackendKind, RelativePath};
use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Write};

/// The owned mutable-marker filename, not an instruction to create or discover it.
pub const MANIFEST_FILE_NAME: &str = "omni-knowledge.json";
/// Required mutable format version; incompatible versions are inspectable but never editable here.
pub const MANIFEST_VERSION: u64 = 2;
/// Current OKF convention spelling recognized by this projection; see the crate's review limits.
///
/// Storage specifies OKF 0.2 but does not enumerate the JSON encoding of `convention_version`.
/// This projection recognizes string `"0.2"`; other encodings remain untouched and read-only.
pub const CONVENTION_VERSION: &str = "0.2";
/// Parser safety ceiling, including the outer object; this is not a storage-format depth promise.
pub const MAX_JSON_DEPTH: usize = 128;

/// Explicit caller-selected finite input/output byte budget, not a fixed manifest-format limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestLimits {
    /// Maximum UTF-8 bytes admitted or emitted; zero admits no manifest.
    pub max_bytes: usize,
}

/// Why an otherwise preserved object cannot produce a mutable candidate through this codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestState {
    /// Version 2 and every required field/semantic supported by this projection.
    EditableV2,
    /// Legacy version 1 is inspection-only; no migration, defaults, or activation occur.
    LegacyV1,
    /// Required manifest version is not supported.
    UnsupportedVersion,
    /// A required known field has an unrecognized semantic value/convention.
    UnsupportedSemantics,
    /// A required recognized field is absent, malformed, duplicated, or inconsistent.
    InvalidKnownFields,
}

/// Exactly one authored text tree; selecting it does not create or rename directories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRoot {
    /// New-library authored tree.
    Pages,
    /// Explicitly mapped retained authored tree, not a second copied pages tree.
    Wiki,
}

/// Intended exposure data, never effective approval or transport activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureMode {
    /// No intended external exposure.
    Off,
    /// Intended read-only exposure within independent local approval.
    ReadOnly,
    /// Intended read/write exposure within independent local approval.
    ReadWrite,
}

/// Intended transport declarations; no listener, client, account, or access grant is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// Owner-only local IPC.
    OwnerIpc,
    /// Owner-launched local stdio.
    OwnerStdio,
    /// Explicitly selected loopback HTTP, still bounded by local approval.
    Http,
}

/// Observed intended exposure fields, excluding service revision/approval state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IntendedExposure {
    /// Intended mode, not the current effective mode.
    pub mode: ExposureMode,
    /// Intended sensitivity classification; no service may infer disclosure approval from it.
    pub sensitive: bool,
    /// Data exclusions that the actual admitting owner must enforce independently.
    pub excluded_paths: Vec<RelativePath>,
    /// Intended transports, none activated by decoding.
    pub transports: Vec<Transport>,
}

/// Intended reconciliation setting, not an executing schedule or proven event capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reconciliation {
    /// Local startup/periodic catch-up.
    StartupPeriodic,
    /// NAS nightly/manual reconciliation.
    NightlyManual,
    /// Explicit manual reconciliation.
    Manual,
}

/// Recognized portable mount declaration; runtime observations and credentials are not its authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MountDeclaration {
    /// Stable nonempty mount data identity, never source identity or permission.
    pub mount_id: String,
    /// Unique single-component logical name; renaming/alias maps are outside this editor.
    pub name: String,
    /// Ordinary backend declaration, not an adapter instance.
    pub backend: BackendKind,
    /// Opaque declared location, never validated/opened as an authorized root here.
    pub location: String,
    /// Intended writable setting, not actual write capability or service approval.
    pub writable: bool,
    /// Intended reconciliation policy, not an active background job.
    pub reconciliation: Reconciliation,
}

/// Recognized display/domain facts only; serializing this snapshot would omit authored extensions.
///
/// This is not a full manifest representation. Persist only a separately validated candidate through
/// the future filesystem owner, not this snapshot or a service DTO.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ManifestMetadata {
    /// Stable nonempty data ID, without a UUID requirement or local root grant.
    pub library_id: String,
    /// Authored display name; preserved without trimming or normalization.
    pub name: String,
    /// The sole authored text tree.
    pub text_root: TextRoot,
    /// Portable declarations, not activated roots or backend observations.
    pub mounts: Vec<MountDeclaration>,
    /// Intended data only; synced expansion does not widen approval.
    pub exposure: IntendedExposure,
}

/// Complete preserved input plus a checked interpretation; no filesystem or permission handle exists.
#[derive(Clone)]
pub struct ManifestDocument {
    /// Exact admitted bytes; never regenerate these for read-only inspection.
    original: Vec<u8>,
    /// Opaque value lexemes used only for the supported name-only candidate rewrite.
    fields: RawObject,
    /// Recognized numeric version shape; no missing-version default is applied.
    version: Option<u64>,
    /// Mutation disposition derived from required fields, never a local approval state.
    state: ManifestState,
    /// Recognized display facts only when every required semantic is supported.
    metadata: Option<ManifestMetadata>,
    /// Original caller byte ceiling, also applied to every candidate.
    limits: ManifestLimits,
}

impl ManifestDocument {
    /// Admit one bounded UTF-8 JSON object; preserve future/legacy semantics for read-only inspection.
    ///
    /// Reject invalid JSON, duplicate outer keys, excessive nesting, or the caller's byte ceiling.
    /// Required nested objects reject duplicate keys during interpretation; unknown optional values
    /// are opaque raw JSON, including exact number spellings and nested extension fields.
    /// Unknown optional fields are not executable features; no critical-field registry is invented.
    pub fn decode(bytes: &[u8], limits: ManifestLimits) -> Result<Self, ManifestError> {
        if bytes.len() > limits.max_bytes {
            return Err(ManifestError::ByteLimit);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::InvalidUtf8)?;
        check_depth(text)?;
        let fields: RawObject =
            serde_json::from_str(text).map_err(|_| ManifestError::InvalidJson)?;
        let version = fields
            .get("version")
            .and_then(|raw| serde_json::from_str(raw).ok());
        let (state, metadata) = match version {
            Some(1) => (ManifestState::LegacyV1, None),
            Some(MANIFEST_VERSION) => match interpret(&fields) {
                Ok(metadata) => (ManifestState::EditableV2, Some(metadata)),
                Err(InterpretError::Unsupported) => (ManifestState::UnsupportedSemantics, None),
                Err(InterpretError::Invalid) => (ManifestState::InvalidKnownFields, None),
            },
            Some(_) => (ManifestState::UnsupportedVersion, None),
            None => (ManifestState::InvalidKnownFields, None),
        };
        Ok(Self {
            original: bytes.to_vec(),
            fields,
            version,
            state,
            metadata,
            limits,
        })
    }

    /// Return original input byte-for-byte, including whitespace, key order, and number spellings.
    ///
    /// This is inspection/copy data, never permission to overwrite or initialize a destination.
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }

    /// Return the parsed unsigned required version, if that field has the recognized numeric shape.
    pub fn version(&self) -> Option<u64> {
        self.version
    }

    /// Return the explicit mutation disposition; read-only never means ready or migrated.
    pub fn state(&self) -> ManifestState {
        self.state
    }

    /// Borrow supported recognized facts; unknown/legacy semantics remain accessible as raw fields.
    pub fn metadata(&self) -> Option<&ManifestMetadata> {
        self.metadata.as_ref()
    }

    /// Borrow one complete raw field value without decoding/normalizing authored extension data.
    pub fn field_json(&self, name: &str) -> Option<&str> {
        self.fields.get(name)
    }

    /// Make a name-only version-2 candidate, never install it or approve any declared setting.
    ///
    /// Refuse every read-only state even for a no-op request. An unchanged name preserves every
    /// original byte. A changed name normalizes outer key encoding/order/spacing, but keeps every other
    /// field's raw JSON value verbatim, including unknown optional nested data and number spellings.
    /// The candidate obeys the same byte/depth limits; a failed edit leaves the original unchanged.
    pub fn with_name(&self, name: &str) -> Result<Self, ManifestError> {
        let Some(metadata) = self.metadata.as_ref() else {
            return Err(ManifestError::ReadOnly);
        };
        if metadata.name == name {
            return Ok(self.clone());
        }
        if name.len() > self.limits.max_bytes {
            return Err(ManifestError::ByteLimit);
        }
        let encoded_name = bounded_encode(name, self.limits.max_bytes)?;
        let encoded_name = String::from_utf8(encoded_name).map_err(|_| ManifestError::Encode)?;
        let mut fields = self.fields.clone();
        fields.0.insert(
            "name".into(),
            RawValue::from_string(encoded_name).map_err(|_| ManifestError::Encode)?,
        );
        let bytes = bounded_encode(&fields, self.limits.max_bytes)?;
        Self::decode(&bytes, self.limits)
    }
}

/// Input-free codec failures; raw Serde errors/source chains are intentionally not exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// Input or candidate exceeds the caller's byte budget.
    ByteLimit,
    /// Input is not UTF-8.
    InvalidUtf8,
    /// Input is not one JSON object, or outer field names are duplicated.
    InvalidJson,
    /// Nesting exceeds the parser safety ceiling, not necessarily a format defect.
    DepthLimit,
    /// Required version/semantics/shape is not editable by this projection.
    ReadOnly,
    /// Bounded candidate encoding failed without exposing authored input.
    Encode,
}

impl fmt::Display for ManifestError {
    /// Return a fixed diagnostic with no rejected field, body, path, or input-bearing source chain.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ByteLimit => "manifest byte budget exhausted",
            Self::InvalidUtf8 => "manifest is not UTF-8",
            Self::InvalidJson => "invalid manifest JSON object",
            Self::DepthLimit => "manifest parser depth exhausted",
            Self::ReadOnly => "manifest mutation is unsupported",
            Self::Encode => "manifest candidate encoding failed",
        })
    }
}

impl std::error::Error for ManifestError {}

/// Raw field values retain precision, nested authored content, and spelling without a JSON AST rewrite.
#[derive(Clone, Serialize)]
#[serde(transparent)]
struct RawObject(BTreeMap<String, Box<RawValue>>);

impl RawObject {
    /// Borrow the exact JSON spelling for a decoded outer field name.
    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(|raw| raw.get())
    }

    /// Read a required recognized field without defaults; raw optional fields remain untouched.
    fn required<T: DeserializeOwned>(&self, name: &str) -> Result<T, InterpretError> {
        serde_json::from_str(self.get(name).ok_or(InterpretError::Invalid)?)
            .map_err(|_| InterpretError::Invalid)
    }
}

impl<'de> Deserialize<'de> for RawObject {
    /// Reject duplicate decoded field names instead of selecting a last-writer interpretation.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(RawObjectVisitor)
    }
}

/// Deserialize only object boundaries; optional values stay opaque validated raw JSON.
struct RawObjectVisitor;

impl<'de> Visitor<'de> for RawObjectVisitor {
    type Value = RawObject;

    /// Describe the shape without embedding input in diagnostics.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique field names")
    }

    /// Collect bounded raw values and reject duplicate keys, including escaped equivalent names.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut fields = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
            if fields.insert(key, value).is_some() {
                return Err(serde::de::Error::custom("duplicate manifest field"));
            }
        }
        Ok(RawObject(fields))
    }
}

/// Distinguish unsupported required meaning from malformed recognized data without exposing values.
enum InterpretError {
    /// Required shape/data is malformed or missing.
    Invalid,
    /// A required semantic tag or convention is unknown.
    Unsupported,
}

/// Interpret only current required fields; never activate a location or invent missing defaults.
fn interpret(fields: &RawObject) -> Result<ManifestMetadata, InterpretError> {
    let convention = fields
        .get("convention_version")
        .ok_or(InterpretError::Invalid)?;
    if serde_json::from_str::<String>(convention).ok().as_deref() != Some(CONVENTION_VERSION) {
        return Err(InterpretError::Unsupported);
    }
    let library_id: String = fields.required("library_id")?;
    if library_id.is_empty() {
        return Err(InterpretError::Invalid);
    }
    let text_root = match fields.required::<String>("text_root")?.as_str() {
        "pages" => TextRoot::Pages,
        "wiki" => TextRoot::Wiki,
        _ => return Err(InterpretError::Unsupported),
    };
    let mut mounts = Vec::new();
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for mount in fields.required::<Vec<RawObject>>("mounts")? {
        let mount_id: String = mount.required("mount_id")?;
        let name: String = mount.required("name")?;
        if mount_id.is_empty()
            || RelativePath::new(name.clone()).is_err()
            || name.contains('/')
            || !ids.insert(mount_id.clone())
            || !names.insert(name.clone())
        {
            return Err(InterpretError::Invalid);
        }
        let backend = match mount.required::<String>("backend")?.as_str() {
            "local" => BackendKind::Local,
            "nas" => BackendKind::Nas,
            "cloud_folder" => BackendKind::CloudFolder,
            "git" => BackendKind::Git,
            _ => return Err(InterpretError::Unsupported),
        };
        let reconciliation = match mount.required::<String>("reconciliation")?.as_str() {
            "startup_periodic" => Reconciliation::StartupPeriodic,
            "nightly_manual" => Reconciliation::NightlyManual,
            "manual" => Reconciliation::Manual,
            _ => return Err(InterpretError::Unsupported),
        };
        mounts.push(MountDeclaration {
            mount_id,
            name,
            backend,
            location: mount.required("location")?,
            writable: mount.required("writable")?,
            reconciliation,
        });
    }
    let exposure: RawObject = fields.required("exposure")?;
    let mode = match exposure.required::<String>("mode")?.as_str() {
        "off" => ExposureMode::Off,
        "read_only" => ExposureMode::ReadOnly,
        "read_write" => ExposureMode::ReadWrite,
        _ => return Err(InterpretError::Unsupported),
    };
    let mut transports = Vec::new();
    for transport in exposure.required::<Vec<String>>("transports")? {
        transports.push(match transport.as_str() {
            "owner_ipc" => Transport::OwnerIpc,
            "owner_stdio" => Transport::OwnerStdio,
            "http" => Transport::Http,
            _ => return Err(InterpretError::Unsupported),
        });
    }
    Ok(ManifestMetadata {
        library_id,
        name: fields.required("name")?,
        text_root,
        mounts,
        exposure: IntendedExposure {
            mode,
            sensitive: exposure.required("sensitive")?,
            excluded_paths: exposure.required("excluded_paths")?,
            transports,
        },
    })
}

/// Bound raw-value parser nesting before parsing/allocating, while ignoring delimiters in strings.
/// JSON syntax validation, including escapes and balanced delimiters, remains Serde's responsibility.
fn check_depth(text: &str) -> Result<(), ManifestError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_JSON_DEPTH {
                        return Err(ManifestError::DepthLimit);
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Private byte-capped sink; no partial candidate is returned on any failure.
struct CandidateWriter {
    /// Private incomplete output, discarded if any encoding step fails.
    bytes: Vec<u8>,
    /// Caller-selected byte ceiling, checked before appending.
    maximum: usize,
    /// Distinguish the fixed budget failure from another encoding failure.
    exceeded: bool,
}

impl Write for CandidateWriter {
    /// Refuse a chunk before extending past the byte budget, including escaped string expansion.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other("manifest byte budget exhausted"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    /// No external sink or buffered effect exists.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Encode a serializable value within the caller's finite budget, returning no partial bytes.
fn bounded_encode<T: Serialize + ?Sized>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, ManifestError> {
    let mut writer = CandidateWriter {
        bytes: Vec::new(),
        maximum,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.exceeded {
            ManifestError::ByteLimit
        } else {
            ManifestError::Encode
        }
    })?;
    Ok(writer.bytes)
}
