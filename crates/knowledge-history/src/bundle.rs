//! Held-file exact-stream, inert native Git bundle verification and object admission.
//! This does not authorize/activate imported refs, configuration, trust, checkout, hooks, filters or remotes.

use super::*;
use knowledge_core::data::ContentDigest;
use std::os::unix::fs::{FileExt as _, OpenOptionsExt};

/// Native bundle refusal, without reflecting input paths/content/tool diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    /// Actual existing executor/context/capacity/unknown-native-owner failure.
    History(Error),
    /// Held file identity/version changed while copying, or bytes do not match the expected exact stream.
    Changed,
    /// Unsupported/malformed bundle header, object graph or native pack verification failure.
    Integrity,
}
impl From<Error> for BundleError {
    fn from(error: Error) -> Self {
        Self::History(error)
    }
}
impl From<std::io::Error> for BundleError {
    fn from(_: std::io::Error) -> Self {
        Self::History(Error::Io)
    }
}
impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Changed => "history bundle changed",
            Self::Integrity => "history bundle integrity",
            Self::History(_) => "history bundle execution",
        })
    }
}
impl std::error::Error for BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::History(error) => Some(error),
            _ => None,
        }
    }
}

/// One qualified advertised app ref. It is inert package data, not permission to create/update this ref.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleRef {
    pub name: String,
    pub version: Oid,
}
/// Actual complete native verification/admission result, not a signature or authority assertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleReport {
    /// Actual copied exact stream digest checked against the caller's expected version.
    pub version: ContentDigest,
    /// Actual held-file stream length.
    pub bytes: u64,
    /// Qualified advertised refs only; no imported remote/configuration is executed.
    pub refs: Vec<BundleRef>,
    /// Actual required immutable prerequisites, validated against this selected native object database.
    pub prerequisites: Vec<Oid>,
    /// True only after actual native object admission; refs/home/config/trust remain untouched in either case.
    pub objects_admitted: bool,
}

impl History {
    /// Verify an already admitted held regular file against its exact stream version, in a private native quarantine.
    /// M5 must first copy its actual VerifiedSource to private owned output and verify that stream before handoff.
    /// An expected digest/file descriptor is data, never a replacement for M5/M6 native current-context permission.
    pub fn verify_bundle(
        &self,
        file: &File,
        expected: &ContentDigest,
        claim: &Claim,
    ) -> Result<BundleReport, BundleError> {
        self.bundle(file, expected, false, claim)
    }

    /// Reverify a held exact stream and admit actual objects only. Does not activate any ref or imported configuration/trust.
    /// Any later selected-ref publication still needs the native host's current context, expected refs and real human approval.
    pub fn admit_bundle(
        &self,
        file: &File,
        expected: &ContentDigest,
        claim: &Claim,
    ) -> Result<BundleReport, BundleError> {
        self.bundle(file, expected, true, claim)
    }

    /// Use the existing executor and one original claim/call across copy, native verify, quarantine unpack and optional admission.
    fn bundle(
        &self,
        file: &File,
        expected: &ContentDigest,
        admit: bool,
        claim: &Claim,
    ) -> Result<BundleReport, BundleError> {
        let call = Call::new(claim)?;
        self.validate()?;
        let _lock = self.lock()?;
        let initial = file.metadata()?;
        if !initial.is_file() {
            return Err(BundleError::Integrity);
        }
        let need = initial
            .len()
            .checked_mul(if admit { 4 } else { 3 })
            .ok_or(Error::Capacity)?;
        if initial.len() == 0 || need > call.bytes_left.get() {
            return Err(Error::Capacity.into());
        }
        let scratch = self.scratch()?;
        let input = scratch.path().join("held-input.bundle");
        let mut output = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&input)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 16 * 1024];
        let mut copied = 0u64;
        while copied < initial.len() {
            call.check()?;
            let bound = buffer
                .len()
                .min(usize::try_from(initial.len() - copied).unwrap_or(usize::MAX));
            let n = file.read_at(&mut buffer[..bound], copied)?;
            if n == 0 {
                return Err(BundleError::Changed);
            }
            call.account_bytes(n)?;
            output.write_all(&buffer[..n])?;
            hasher.update(&buffer[..n]);
            copied += n as u64;
        }
        if version_tuple(&initial) != version_tuple(&file.metadata()?) {
            return Err(BundleError::Changed);
        }
        let actual = ContentDigest::new(format!("{:x}", hasher.finalize()))
            .map_err(|_| BundleError::Integrity)?;
        if &actual != expected {
            return Err(BundleError::Changed);
        }
        output.sync_all()?;
        let (refs, prerequisites) = header(&output, &call)?;
        let quarantine = scratch.path().join("quarantine.git");
        fs::create_dir(&quarantine)?;
        run(
            &quarantine,
            scratch.path(),
            &["init", "--bare", "--object-format=sha1", "--template=", "."],
            b"",
            None,
            &call,
            (None, &self.runtime),
        )?;
        fs::write(quarantine.join("config"), CONFIG)?;
        // This native-created alternate points only to the already admitted selected local object database, never imported config.
        let objects = self.repo.join("objects");
        let objects = objects.to_str().ok_or(Error::Invalid)?;
        if objects.contains(['\n', '\r']) {
            return Err(BundleError::Integrity);
        }
        fs::write(
            quarantine.join("objects/info/alternates"),
            format!("{objects}\n"),
        )?;
        let path = input.to_str().ok_or(Error::Invalid)?;
        native_bundle(run(
            &quarantine,
            scratch.path(),
            &["bundle", "verify", path],
            b"",
            None,
            &call,
            (None, &self.runtime),
        ))?;
        call.account_bytes(usize::try_from(copied).map_err(|_| Error::Capacity)?)?;
        native_bundle(run(
            &quarantine,
            scratch.path(),
            &[
                "-c",
                "transfer.fsckObjects=true",
                "-c",
                "fetch.fsckObjects=true",
                "bundle",
                "unbundle",
                path,
            ],
            b"",
            None,
            &call,
            (None, &self.runtime),
        ))?;
        for advertised in &refs {
            let kind = native_bundle(run(
                &quarantine,
                scratch.path(),
                &["cat-file", "-t", advertised.version.as_str()],
                b"",
                None,
                &call,
                (None, &self.runtime),
            ))?;
            if kind != b"commit\n" {
                return Err(BundleError::Integrity);
            }
        }
        if version_tuple(&initial) != version_tuple(&file.metadata()?) {
            return Err(BundleError::Changed);
        }
        self.validate()?;
        if admit {
            call.account_bytes(usize::try_from(copied).map_err(|_| Error::Capacity)?)?;
            native_bundle(self.git(
                &[
                    "-c",
                    "transfer.fsckObjects=true",
                    "-c",
                    "fetch.fsckObjects=true",
                    "bundle",
                    "unbundle",
                    path,
                ],
                b"",
                scratch.path(),
                None,
                &call,
            ))?;
        }
        Ok(BundleReport {
            version: actual,
            bytes: copied,
            refs,
            prerequisites,
            objects_admitted: admit,
        })
    }
}

/// Never downgrade an actual unsettled native owner/deadline/capacity failure into a malformed-input result.
fn native_bundle(result: Result<Vec<u8>, Error>) -> Result<Vec<u8>, BundleError> {
    result.map_err(|error| match error {
        Error::Git => BundleError::Integrity,
        error => BundleError::History(error),
    })
}
/// Native descriptor version tuple is an observation guard, not authority reconstructed from a path/hash.
fn version_tuple(meta: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

/// Parse only a bounded bundle header, never dump arbitrary refs or interpret URL/config/provider data.
fn header(file: &File, call: &Call) -> Result<(Vec<BundleRef>, Vec<Oid>), BundleError> {
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    let mut block = [0u8; 1024];
    let end = loop {
        call.check()?;
        let n = file.read_at(&mut block, offset)?;
        if n == 0 {
            return Err(BundleError::Integrity);
        }
        if bytes.len().checked_add(n).ok_or(Error::Capacity)? > call.limit {
            return Err(Error::Capacity.into());
        }
        bytes.extend_from_slice(&block[..n]);
        offset += n as u64;
        if let Some(end) = bytes.windows(2).position(|v| v == b"\n\n") {
            break end;
        }
    };
    call.account_bytes(end + 2)?;
    let text = std::str::from_utf8(&bytes[..end]).map_err(|_| BundleError::Integrity)?;
    let mut lines = text.lines();
    match lines.next() {
        Some("# v2 git bundle" | "# v3 git bundle") => {}
        _ => return Err(BundleError::Integrity),
    }
    let mut refs = Vec::new();
    let mut prerequisites = Vec::new();
    for line in lines {
        call.account_files(1)?;
        if line == "@object-format=sha1" {
            continue;
        }
        if line.starts_with('@') {
            return Err(BundleError::Integrity);
        }
        if let Some(value) = line.strip_prefix('-') {
            let oid = value.split_once(' ').map(|(oid, _)| oid).unwrap_or(value);
            prerequisites.push(Oid::parse(oid).map_err(|_| BundleError::Integrity)?);
            continue;
        }
        let (oid, name) = line.split_once(' ').ok_or(BundleError::Integrity)?;
        if name != ACCEPTED {
            let id = [
                "refs/heads/omni-proposals/",
                "refs/omni/prepared/",
                "refs/omni/operations/",
            ]
            .iter()
            .find_map(|prefix| name.strip_prefix(prefix))
            .ok_or(BundleError::Integrity)?;
            validate_id(id).map_err(|_| BundleError::Integrity)?;
        }
        if refs.iter().any(|entry: &BundleRef| entry.name == name) {
            return Err(BundleError::Integrity);
        }
        refs.push(BundleRef {
            name: name.to_owned(),
            version: Oid::parse(oid).map_err(|_| BundleError::Integrity)?,
        });
    }
    if refs.is_empty() {
        return Err(BundleError::Integrity);
    }
    Ok((refs, prerequisites))
}
