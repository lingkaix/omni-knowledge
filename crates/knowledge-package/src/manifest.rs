//! Canonical package manifest. Snapshot identity is the SHA-256 of these bytes.
use crate::{
    Availability, BlobRef, Coverage, DigestState, EntryRecord, Error, ExternalRef, FORMAT,
    IndexCoverage, IndexSet, Manifest, Method, MountFact, Preset, Role, Window,
};
use knowledge_core::data::ContentDigest;
use knowledge_core::data::RelativePath;
use serde_json::{Map, Number, Value};

pub(crate) fn emit(manifest: &Manifest, work: &mut crate::Work<'_>) -> Result<Vec<u8>, Error> {
    let bound = crate::metadata::manifest(manifest).bound()?;
    let _scratch = crate::preallocate(work, bound)?;
    crate::charge_fresh(work, bound, 0)?;
    let mut map = Map::new();
    map.insert("capture_window".into(), window(&manifest.window)?);
    map.insert("coverage".into(), coverage(&manifest.coverage)?);
    map.insert(
        "entries".into(),
        Value::Array(
            manifest
                .entries
                .iter()
                .map(entry)
                .collect::<Result<_, _>>()?,
        ),
    );
    map.insert(
        "external_references".into(),
        Value::Array(
            manifest
                .external
                .iter()
                .map(external)
                .collect::<Result<_, _>>()?,
        ),
    );
    map.insert("format".into(), Value::String(FORMAT.into()));
    map.insert("history".into(), blob(manifest.history.as_ref())?);
    map.insert("indexes".into(), indexes(manifest.indexes.as_ref())?);
    map.insert("journal".into(), blob(manifest.journal.as_ref())?);
    map.insert(
        "library_id".into(),
        Value::String(checked_token(&manifest.library_id)?),
    );
    map.insert(
        "mounts".into(),
        Value::Array(
            manifest
                .mounts
                .iter()
                .map(mount)
                .collect::<Result<_, _>>()?,
        ),
    );
    map.insert("name".into(), Value::String(checked_token(&manifest.name)?));
    map.insert(
        "preset".into(),
        Value::String(preset(manifest.preset).into()),
    );
    map.insert(
        "required_features".into(),
        Value::Array(
            manifest
                .required_features
                .iter()
                .map(|f| Ok(Value::String(checked_token(f)?)))
                .collect::<Result<_, _>>()?,
        ),
    );
    map.insert(
        "text_root".into(),
        Value::String(text_root(&manifest.text_root)?.into()),
    );
    map.insert(
        "version".into(),
        Value::Number(Number::from(manifest.version)),
    );
    for (key, value) in &manifest.extra {
        if map.contains_key(key) {
            return Err(Error::Invalid);
        }
        map.insert(key.clone(), value.clone());
    }
    canonical_bytes(&Value::Object(map))
}

pub(crate) fn parse(bytes: &[u8], work: &mut crate::Work<'_>) -> Result<Manifest, Error> {
    if bytes.is_empty() {
        return Err(Error::Invalid);
    }
    let _scratch = crate::metadata::parsing(bytes, work)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    if value_depth(&value) > 32 {
        return Err(Error::Invalid);
    }
    let again = canonical_bytes(&value)?;
    if again.as_slice() != bytes {
        return Err(Error::Invalid);
    }
    interpret(value)
}

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, Error> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Invalid)
}

fn value_depth(value: &Value) -> u32 {
    match value {
        Value::Array(items) => 1 + items.iter().map(value_depth).max().unwrap_or(0),
        Value::Object(map) => 1 + map.values().map(value_depth).max().unwrap_or(0),
        _ => 1,
    }
}

fn interpret(value: Value) -> Result<Manifest, Error> {
    let mut obj = match value {
        Value::Object(map) => map,
        _ => return Err(Error::Invalid),
    };
    let format = take_str(&mut obj, "format")?;
    if format != FORMAT {
        return Err(Error::Unsupported);
    }
    let version = take_u32(&mut obj, "version")?;
    if version != 1 {
        return Err(Error::Unsupported);
    }
    let library_id = checked_token(&take_str(&mut obj, "library_id")?)?;
    let name = checked_token(&take_str(&mut obj, "name")?)?;
    let text_root = text_root(&take_str(&mut obj, "text_root")?)?.to_owned();
    let preset = parse_preset(&take_str(&mut obj, "preset")?)?;
    let required_features = take_array(&mut obj, "required_features")?
        .into_iter()
        .map(as_token)
        .collect::<Result<Vec<_>, _>>()?;
    let mounts = take_array(&mut obj, "mounts")?
        .into_iter()
        .map(parse_mount)
        .collect::<Result<Vec<_>, _>>()?;
    let entries = take_array(&mut obj, "entries")?
        .into_iter()
        .map(parse_entry)
        .collect::<Result<Vec<_>, _>>()?;
    if entries
        .windows(2)
        .any(|pair| pair[0].path.as_str() >= pair[1].path.as_str())
    {
        return Err(Error::Invalid);
    }
    let external = take_array(&mut obj, "external_references")?
        .into_iter()
        .map(parse_external)
        .collect::<Result<Vec<_>, _>>()?;
    if external.windows(2).any(|pair| {
        (pair[0].source_id.as_str(), pair[0].path.as_str())
            >= (pair[1].source_id.as_str(), pair[1].path.as_str())
    }) {
        return Err(Error::Invalid);
    }
    let indexes = parse_indexes(obj.remove("indexes").ok_or(Error::Invalid)?)?;
    let history = parse_blob(obj.remove("history").ok_or(Error::Invalid)?)?;
    let journal = parse_blob(obj.remove("journal").ok_or(Error::Invalid)?)?;
    let window = parse_window(obj.remove("capture_window").ok_or(Error::Invalid)?)?;
    let coverage = parse_coverage(obj.remove("coverage").ok_or(Error::Invalid)?)?;
    Ok(Manifest {
        version,
        library_id,
        name,
        text_root,
        preset,
        required_features,
        mounts,
        entries,
        external,
        indexes,
        history,
        journal,
        window,
        coverage,
        extra: obj,
    })
}

fn window(window: &Window) -> Result<Value, Error> {
    stamp(&window.start)?;
    stamp(&window.end)?;
    if window.start.as_str() > window.end.as_str() {
        return Err(Error::Invalid);
    }
    let mut map = Map::new();
    map.insert("end".into(), Value::String(window.end.clone()));
    map.insert("start".into(), Value::String(window.start.clone()));
    Ok(Value::Object(map))
}

fn parse_window(value: Value) -> Result<Window, Error> {
    let mut obj = object(value)?;
    let start = take_str(&mut obj, "start")?;
    let end = take_str(&mut obj, "end")?;
    if !obj.is_empty() {
        return Err(Error::Invalid);
    }
    stamp(&start)?;
    stamp(&end)?;
    if start.as_str() > end.as_str() {
        return Err(Error::Invalid);
    }
    Ok(Window { start, end })
}

fn stamp(value: &str) -> Result<(), Error> {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[10] != b'T' || bytes[19] != b'Z' {
        return Err(Error::Invalid);
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !digits
        .into_iter()
        .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn coverage(coverage: &Coverage) -> Result<Value, Error> {
    let mut map = Map::new();
    map.insert(
        "indexes".into(),
        Value::String(index_coverage(coverage.indexes).into()),
    );
    map.insert("omitted".into(), dec(coverage.omitted));
    map.insert("pending".into(), dec(coverage.pending));
    map.insert("selected".into(), dec(coverage.selected));
    map.insert("unavailable".into(), dec(coverage.unavailable));
    map.insert("unverified".into(), dec(coverage.unverified));
    Ok(Value::Object(map))
}

fn parse_coverage(value: Value) -> Result<Coverage, Error> {
    let mut obj = object(value)?;
    let indexes = parse_index_coverage(&take_str(&mut obj, "indexes")?)?;
    let omitted = take_dec(&mut obj, "omitted")?;
    let pending = take_dec(&mut obj, "pending")?;
    let selected = take_dec(&mut obj, "selected")?;
    let unavailable = take_dec(&mut obj, "unavailable")?;
    let unverified = take_dec(&mut obj, "unverified")?;
    if !obj.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(Coverage {
        selected,
        omitted,
        unverified,
        pending,
        unavailable,
        indexes,
    })
}

fn entry(entry: &EntryRecord) -> Result<Value, Error> {
    let mut map = Map::new();
    map.insert("method".into(), Value::String(method(entry.method).into()));
    map.insert("mount_id".into(), option_token(entry.mount_id.as_deref())?);
    map.insert("path".into(), Value::String(entry.path.as_str().to_owned()));
    map.insert("role".into(), Value::String(role(entry.role).into()));
    map.insert(
        "sha256".into(),
        Value::String(entry.sha256.as_str().to_owned()),
    );
    map.insert("size_bytes".into(), dec(entry.size_bytes));
    map.insert(
        "source_id".into(),
        option_token(entry.source_id.as_deref())?,
    );
    Ok(Value::Object(map))
}

fn parse_entry(value: Value) -> Result<EntryRecord, Error> {
    let mut obj = object(value)?;
    let method = parse_method(&take_str(&mut obj, "method")?)?;
    let mount_id = take_option_token(&mut obj, "mount_id")?;
    let path = path(&take_str(&mut obj, "path")?)?;
    let role = parse_role(&take_str(&mut obj, "role")?)?;
    let sha256 = ContentDigest::new(take_str(&mut obj, "sha256")?).map_err(|_| Error::Invalid)?;
    let size_bytes = take_dec(&mut obj, "size_bytes")?;
    let source_id = take_option_token(&mut obj, "source_id")?;
    if !obj.is_empty() {
        return Err(Error::Invalid);
    }
    if matches!(
        role,
        Role::Original | Role::Index | Role::History | Role::Journal
    ) && method != Method::Stored
    {
        return Err(Error::Unsupported);
    }
    if role == Role::Core && method != Method::Deflate {
        return Err(Error::Unsupported);
    }
    if role == Role::Original && (mount_id.is_none() || source_id.is_none()) {
        return Err(Error::Invalid);
    }
    Ok(EntryRecord {
        path,
        size_bytes,
        sha256,
        method,
        role,
        mount_id,
        source_id,
    })
}

fn external(external: &ExternalRef) -> Result<Value, Error> {
    let mut map = Map::new();
    map.insert(
        "availability".into(),
        Value::String(availability(external.availability).into()),
    );
    map.insert(
        "digest".into(),
        match &external.digest {
            Some(digest) => Value::String(digest.as_str().to_owned()),
            None => Value::Null,
        },
    );
    map.insert(
        "digest_state".into(),
        Value::String(digest_state(external.digest_state).into()),
    );
    map.insert(
        "mount_id".into(),
        Value::String(checked_token(&external.mount_id)?),
    );
    map.insert(
        "path".into(),
        Value::String(external.path.as_str().to_owned()),
    );
    map.insert(
        "size_bytes".into(),
        match external.size_bytes {
            Some(size) => dec(size),
            None => Value::Null,
        },
    );
    map.insert(
        "source_id".into(),
        Value::String(checked_token(&external.source_id)?),
    );
    Ok(Value::Object(map))
}

fn parse_external(value: Value) -> Result<ExternalRef, Error> {
    let mut obj = object(value)?;
    let availability = parse_availability(&take_str(&mut obj, "availability")?)?;
    let digest = match obj.remove("digest").ok_or(Error::Invalid)? {
        Value::Null => None,
        Value::String(text) => Some(ContentDigest::new(text).map_err(|_| Error::Invalid)?),
        _ => return Err(Error::Invalid),
    };
    let digest_state = parse_digest_state(&take_str(&mut obj, "digest_state")?)?;
    let mount_id = checked_token(&take_str(&mut obj, "mount_id")?)?;
    let path = path(&take_str(&mut obj, "path")?)?;
    let size_bytes = match obj.remove("size_bytes").ok_or(Error::Invalid)? {
        Value::Null => None,
        Value::String(text) => Some(parse_dec(&text)?),
        _ => return Err(Error::Invalid),
    };
    let source_id = checked_token(&take_str(&mut obj, "source_id")?)?;
    if !obj.is_empty() {
        return Err(Error::Invalid);
    }
    if digest_state == DigestState::Verified && digest.is_none() {
        return Err(Error::Invalid);
    }
    Ok(ExternalRef {
        source_id,
        mount_id,
        path,
        digest_state,
        digest,
        availability,
        size_bytes,
    })
}

fn mount(mount: &MountFact) -> Result<Value, Error> {
    let mut map = Map::new();
    map.insert(
        "backend".into(),
        Value::String(backend(&mount.backend)?.into()),
    );
    map.insert("location".into(), Value::String(location(&mount.location)?));
    map.insert(
        "mount_id".into(),
        Value::String(checked_token(&mount.mount_id)?),
    );
    map.insert("name".into(), Value::String(checked_token(&mount.name)?));
    Ok(Value::Object(map))
}

fn parse_mount(value: Value) -> Result<MountFact, Error> {
    let mut obj = object(value)?;
    let backend = backend(&take_str(&mut obj, "backend")?)?.to_owned();
    let location = location(&take_str(&mut obj, "location")?)?;
    let mount_id = checked_token(&take_str(&mut obj, "mount_id")?)?;
    let name = checked_token(&take_str(&mut obj, "name")?)?;
    if !obj.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(MountFact {
        mount_id,
        name,
        backend,
        location,
    })
}

fn indexes(indexes: Option<&IndexSet>) -> Result<Value, Error> {
    let Some(indexes) = indexes else {
        return Ok(Value::Null);
    };
    let mut map = Map::new();
    map.insert("compatible".into(), Value::Bool(indexes.compatible));
    map.insert(
        "paths".into(),
        Value::Array(
            indexes
                .paths
                .iter()
                .map(|path| Ok(Value::String(path.as_str().to_owned())))
                .collect::<Result<_, _>>()?,
        ),
    );
    if let Some(reason) = &indexes.fallback {
        map.insert("reason".into(), Value::String(checked_token(reason)?));
    }
    Ok(Value::Object(map))
}

fn parse_indexes(value: Value) -> Result<Option<IndexSet>, Error> {
    match value {
        Value::Null => Ok(None),
        Value::Object(mut obj) => {
            let compatible = match obj.remove("compatible") {
                Some(Value::Bool(value)) => value,
                _ => return Err(Error::Invalid),
            };
            let paths = match obj.remove("paths") {
                Some(Value::Array(items)) => items
                    .into_iter()
                    .map(as_path)
                    .collect::<Result<Vec<_>, _>>()?,
                _ => return Err(Error::Invalid),
            };
            let declared = match obj.remove("reason") {
                None => None,
                Some(Value::String(reason)) if !reason.is_empty() => Some(reason),
                _ => return Err(Error::Invalid),
            };
            if !compatible && declared.is_none() {
                return Err(Error::Invalid);
            }
            if !obj.is_empty() {
                return Err(Error::Invalid);
            }
            // Compatible generations still need the kernel Directory provider.
            // Until that query exists, core bytes stay usable and the index is omitted.
            let fallback = if compatible {
                Some(declared.unwrap_or_else(|| "kernel_not_wired".into()))
            } else {
                declared
            };
            Ok(Some(IndexSet {
                paths,
                compatible,
                fallback,
            }))
        }
        _ => Err(Error::Invalid),
    }
}

fn blob(blob: Option<&BlobRef>) -> Result<Value, Error> {
    let Some(blob) = blob else {
        return Ok(Value::Null);
    };
    let mut map = Map::new();
    map.insert("path".into(), Value::String(blob.path.as_str().to_owned()));
    map.insert("scope".into(), Value::String(checked_token(&blob.scope)?));
    Ok(Value::Object(map))
}

fn parse_blob(value: Value) -> Result<Option<BlobRef>, Error> {
    match value {
        Value::Null => Ok(None),
        Value::Object(mut obj) => {
            let path = path(&take_str(&mut obj, "path")?)?;
            let scope = checked_token(&take_str(&mut obj, "scope")?)?;
            if !obj.is_empty() {
                return Err(Error::Invalid);
            }
            Ok(Some(BlobRef { path, scope }))
        }
        _ => Err(Error::Invalid),
    }
}

fn object(value: Value) -> Result<Map<String, Value>, Error> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Error::Invalid),
    }
}

fn take_str(map: &mut Map<String, Value>, key: &str) -> Result<String, Error> {
    match map.remove(key) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(Error::Invalid),
    }
}

fn take_u32(map: &mut Map<String, Value>, key: &str) -> Result<u32, Error> {
    match map.remove(key) {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(Error::Invalid),
        _ => Err(Error::Invalid),
    }
}

fn take_dec(map: &mut Map<String, Value>, key: &str) -> Result<u64, Error> {
    parse_dec(&take_str(map, key)?)
}

fn parse_dec(text: &str) -> Result<u64, Error> {
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Error::Invalid);
    }
    text.parse().map_err(|_| Error::Invalid)
}

fn dec(value: u64) -> Value {
    Value::String(value.to_string())
}

fn take_array(map: &mut Map<String, Value>, key: &str) -> Result<Vec<Value>, Error> {
    match map.remove(key) {
        Some(Value::Array(items)) => Ok(items),
        _ => Err(Error::Invalid),
    }
}

fn take_option_token(map: &mut Map<String, Value>, key: &str) -> Result<Option<String>, Error> {
    match map.remove(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(checked_token(&value)?)),
        _ => Err(Error::Invalid),
    }
}

fn option_token(value: Option<&str>) -> Result<Value, Error> {
    match value {
        Some(value) => Ok(Value::String(checked_token(value)?)),
        None => Ok(Value::Null),
    }
}

fn as_token(value: Value) -> Result<String, Error> {
    match value {
        Value::String(value) => checked_token(&value),
        _ => Err(Error::Invalid),
    }
}

fn as_path(value: Value) -> Result<RelativePath, Error> {
    match value {
        Value::String(value) => path(&value),
        _ => Err(Error::Invalid),
    }
}

pub(crate) fn path(value: &str) -> Result<RelativePath, Error> {
    let path = RelativePath::new(value).map_err(|_| Error::Hostile)?;
    if forbidden(path.as_str()) {
        return Err(Error::Forbidden);
    }
    Ok(path)
}

pub(crate) fn forbidden(path: &str) -> bool {
    path.split('/').any(|segment| {
        matches!(
            segment,
            ".git" | ".omni" | ".ssh" | ".gnupg" | "credentials"
        )
    })
}

pub(crate) fn checked_token(value: &str) -> Result<String, Error> {
    if value.is_empty()
        || value.len() > 256
        || value
            .chars()
            .any(|char| char.is_control() || char == '/' || char == '\\')
    {
        return Err(Error::Invalid);
    }
    Ok(value.to_owned())
}

fn location(value: &str) -> Result<String, Error> {
    if value.is_empty()
        || value.len() > 1024
        || value.chars().any(|char| char.is_control())
        || value.contains("://") && value.contains('@')
    {
        return Err(Error::Invalid);
    }
    let lower = value.to_ascii_lowercase();
    if lower.contains("password") || lower.contains("token=") || lower.contains("secret") {
        return Err(Error::Forbidden);
    }
    Ok(value.to_owned())
}

fn text_root(value: &str) -> Result<&str, Error> {
    match value {
        "pages" | "wiki" => Ok(value),
        _ => Err(Error::Invalid),
    }
}

fn backend(value: &str) -> Result<&str, Error> {
    match value {
        "local" | "nas" | "cloud_folder" | "git" => Ok(value),
        _ => Err(Error::Invalid),
    }
}

fn preset(preset: Preset) -> &'static str {
    match preset {
        Preset::Frozen => "frozen",
        Preset::Backup => "backup",
        Preset::Migration => "migration",
    }
}

fn parse_preset(value: &str) -> Result<Preset, Error> {
    match value {
        "frozen" => Ok(Preset::Frozen),
        "backup" => Ok(Preset::Backup),
        "migration" => Ok(Preset::Migration),
        _ => Err(Error::Invalid),
    }
}

fn method(method: Method) -> &'static str {
    match method {
        Method::Stored => "stored",
        Method::Deflate => "deflate",
    }
}

fn parse_method(value: &str) -> Result<Method, Error> {
    match value {
        "stored" => Ok(Method::Stored),
        "deflate" => Ok(Method::Deflate),
        _ => Err(Error::Invalid),
    }
}

fn role(role: Role) -> &'static str {
    match role {
        Role::Core => "core",
        Role::Original => "original",
        Role::Index => "index",
        Role::History => "history",
        Role::Journal => "journal",
    }
}

fn parse_role(value: &str) -> Result<Role, Error> {
    match value {
        "core" => Ok(Role::Core),
        "original" => Ok(Role::Original),
        "index" => Ok(Role::Index),
        "history" => Ok(Role::History),
        "journal" => Ok(Role::Journal),
        _ => Err(Error::Invalid),
    }
}

fn digest_state(state: DigestState) -> &'static str {
    match state {
        DigestState::Verified => "verified",
        DigestState::Unverified => "unverified",
        DigestState::Pending => "pending",
    }
}

fn parse_digest_state(value: &str) -> Result<DigestState, Error> {
    match value {
        "verified" => Ok(DigestState::Verified),
        "unverified" => Ok(DigestState::Unverified),
        "pending" => Ok(DigestState::Pending),
        _ => Err(Error::Invalid),
    }
}

fn availability(value: Availability) -> &'static str {
    match value {
        Availability::Available => "available",
        Availability::Unavailable => "unavailable",
    }
}

fn parse_availability(value: &str) -> Result<Availability, Error> {
    match value {
        "available" => Ok(Availability::Available),
        "unavailable" => Ok(Availability::Unavailable),
        _ => Err(Error::Invalid),
    }
}

fn index_coverage(value: IndexCoverage) -> &'static str {
    match value {
        IndexCoverage::None => "none",
        IndexCoverage::Omitted => "omitted",
        IndexCoverage::Complete => "complete",
    }
}

fn parse_index_coverage(value: &str) -> Result<IndexCoverage, Error> {
    match value {
        "none" => Ok(IndexCoverage::None),
        "omitted" => Ok(IndexCoverage::Omitted),
        "complete" => Ok(IndexCoverage::Complete),
        _ => Err(Error::Invalid),
    }
}

pub(crate) fn operation_id(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
