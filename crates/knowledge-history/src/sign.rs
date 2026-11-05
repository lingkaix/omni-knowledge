//! Native SSH signing/verification. Public material is data; only the native caller selects trust/providers.

use super::*;
use std::os::unix::process::ExitStatusExt;
use std::os::unix::{
    ffi::OsStrExt,
    fs::{FileExt as _, PermissionsExt},
};

/// Selected Git metadata, not an authentication credential or a wire permission grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}
impl GitIdentity {
    /// Validate native selected identity without reading Git/user configuration.
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Result<Self, Error> {
        let value = Self {
            name: name.into(),
            email: email.into(),
        };
        validate_identity(&value)?;
        Ok(value)
    }
}

/// Actual cryptographic result, independent of author/trailer claims and key-provider availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureState {
    ValidKnown,
    ValidUnknown,
    Invalid,
    Unsigned,
}
/// Actual verification facts; an imported public key never grants known trust by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verification {
    pub state: SignatureState,
    pub principal: Option<String>,
    pub fingerprint: Option<String>,
}

/// One explicit native user's trusted public signer choice; never deserialize this from a package/wire DTO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownSigner {
    principal: String,
    public_key: String,
}
impl KnownSigner {
    /// Select exact public material/principal; the native host, not this data constructor, owns human authorization.
    pub fn selected(principal: &str, public_key: &str) -> Result<Self, Error> {
        if principal.is_empty()
            || principal.len() > 256
            || !principal
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            principal: principal.to_owned(),
            public_key: public_material(public_key)?,
        })
    }
}

/// Selected system-agent identity. Contains public material and an explicitly supplied native socket, never a private key.
#[derive(Clone)]
pub struct SshSigner {
    public_key: String,
    fingerprint: String,
    socket: PathBuf,
}
impl std::fmt::Debug for SshSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SshSigner")
    }
}
impl SshSigner {
    /// Read one already selected public descriptor, without its cursor or ambient SSH_AUTH_SOCK/configuration.
    pub fn select_agent(public_key: &File, socket: &Path, claim: &Claim) -> Result<Self, Error> {
        if !public_key.metadata()?.is_file()
            || !socket.is_absolute()
            || socket.as_os_str().as_bytes().contains(&0)
        {
            return Err(Error::Invalid);
        }
        let mut bytes = [0u8; 4097];
        let n = public_key.read_at(&mut bytes, 0)?;
        if n > 4096 {
            return Err(Error::Capacity);
        }
        let public_key =
            public_material(std::str::from_utf8(&bytes[..n]).map_err(|_| Error::Invalid)?)?;
        let runtime = native::Runtime::default();
        let scratch = native::Scratch::new(&std::env::temp_dir(), &runtime)?;
        let call = Call::new(claim)?;
        let path = scratch.path().join("selected.pub");
        fs::write(&path, &public_key)?;
        let output = tool(
            &runtime,
            scratch.path(),
            "/usr/bin/ssh-keygen",
            &[
                "-l",
                "-E",
                "sha256",
                "-f",
                path.to_str().ok_or(Error::Invalid)?,
            ],
            b"",
            None,
            &call,
        )?;
        if !output.status.success() {
            return Err(Error::Invalid);
        }
        let fingerprint = std::str::from_utf8(&output.stdout)
            .map_err(|_| Error::Recovery)?
            .split_whitespace()
            .nth(1)
            .filter(|v| v.starts_with("SHA256:"))
            .ok_or(Error::Recovery)?
            .to_owned();
        Ok(Self {
            public_key,
            fingerprint,
            socket: socket.to_owned(),
        })
    }
    /// Public native selected fingerprint, not private bytes or an authentication assertion about a commit author.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Public material for an explicit native known-signer choice, never automatic package trust.
    pub fn public_key(&self) -> &str {
        &self.public_key
    }
}

/// First-use protection is an explicit native choice; no model/default unsigned or unencrypted fallback exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyProtection {
    NativePrompt,
    UnencryptedExplicit,
}

/// Public pinned signing selection; no key-provider socket/path/private bytes enter an operation recipe.
#[derive(Serialize, Deserialize)]
struct Selection {
    public_key: String,
    fingerprint: String,
}

impl History {
    /// Attach only a native selected system-agent provider; never read ambient credentials or change library mode.
    pub fn with_signer(mut self, signer: SshSigner) -> Self {
        self.signer = Some(signer);
        self
    }
    /// Select a distinct actual committer for new operations; retries retain their recorded original committer.
    pub fn with_committer(mut self, identity: GitIdentity) -> Result<Self, Error> {
        validate_identity(&identity)?;
        self.committer = Some(identity);
        Ok(self)
    }

    /// Explicit native first-use system-key setup, outside the library/history/cache. Never invoked by discovery or model flags.
    /// NativePrompt requires a real native terminal/provider; absence fails rather than fabricating prompt success.
    pub fn setup_system_signer(
        &self,
        destination: &Path,
        socket: &Path,
        protection: KeyProtection,
        claim: &Claim,
    ) -> Result<SshSigner, Error> {
        use std::os::unix::fs::DirBuilderExt;
        self.validate()?;
        if self.owner.agent.is_some()
            || !destination.is_absolute()
            || destination.starts_with(&self.home)
            || destination.starts_with(&self.repo)
        {
            return Err(Error::ContextMismatch);
        }
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));
        if cache.is_some_and(|cache| destination.starts_with(cache)) {
            return Err(Error::Invalid);
        }
        if protection == KeyProtection::NativePrompt {
            use std::io::IsTerminal;
            if !std::io::stdin().is_terminal() {
                return Err(Error::SigningRequired);
            }
        }
        let parent = selected_directory(destination.parent().ok_or(Error::Invalid)?)?;
        let destination = parent.join(destination.file_name().ok_or(Error::Invalid)?);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination)?;
        let path = destination.join("identity");
        let public_path = destination.join("identity.pub");
        let call = Call::new(claim)?;
        let scratch = self.scratch()?;
        let mut args = vec![
            "-q",
            "-t",
            "ed25519",
            "-C",
            "",
            "-f",
            path.to_str().ok_or(Error::Invalid)?,
        ];
        if protection == KeyProtection::UnencryptedExplicit {
            args.extend(["-N", ""]);
        }
        let generated = tool(
            &self.runtime,
            scratch.path(),
            "/usr/bin/ssh-keygen",
            &args,
            b"",
            Some(socket),
            &call,
        )?;
        if !generated.status.success() {
            return Err(Error::SigningRequired);
        }
        for path in [&path, &public_path] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
            File::open(path)?.sync_all()?;
        }
        File::open(&destination)?.sync_all()?;
        let added = tool(
            &self.runtime,
            scratch.path(),
            "/usr/bin/ssh-add",
            &["-q", path.to_str().ok_or(Error::Invalid)?],
            b"",
            Some(socket),
            &call,
        )?;
        if !added.status.success() {
            return Err(Error::SigningRequired);
        }
        SshSigner::select_agent(&File::open(public_path)?, socket, claim)
    }

    /// Verify a scoped actual immutable version with explicit native public trust, without a signing agent/keychain.
    pub fn verify_signature(
        &self,
        version: &Oid,
        known: &[KnownSigner],
        claim: &Claim,
    ) -> Result<Verification, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = self.scratch()?;
        self.qualify_version(version, scratch.path(), &call)?;
        let actual = self.git(
            &["cat-file", "commit", version.as_str()],
            b"",
            scratch.path(),
            None,
            &call,
        )?;
        verify(&self.runtime, &actual, known, scratch.path(), &call)
    }

    /// Deterministic unsigned payload; its exact persisted signed bytes are retried without requesting another signature.
    pub(super) fn commit_payload(&self, intent: &Intent) -> Vec<u8> {
        let mut payload = format!("tree {}\n", intent.prepared.tree.as_str());
        for parent in &intent.parents {
            payload.push_str(&format!("parent {}\n", parent.as_str()));
        }
        let committer = intent.committer.clone().unwrap_or(GitIdentity {
            name: self.owner.name.clone(),
            email: self.owner.email.clone(),
        });
        payload.push_str(&format!(
            "author {} <{}> {} +0000\ncommitter {} <{}> {} +0000\n\n",
            self.owner.name,
            self.owner.email,
            intent.seconds,
            committer.name,
            committer.email,
            intent.seconds
        ));
        payload.push_str(&self.commit_message(intent));
        payload.into_bytes()
    }

    /// Retain exact public selection and actual signed object before native object installation/ref publication.
    pub(super) fn signed_candidate(
        &self,
        intent: &Intent,
        scratch: &Path,
        call: &Call,
    ) -> Result<Oid, Error> {
        let selection_path = self
            .repo
            .join("omni-state/operations")
            .join(format!("{}.signer.json", intent.prepared.operation_id));
        let selection: Selection = if selection_path.exists() {
            load(&selection_path)?
        } else {
            let signer = self.signer.as_ref().ok_or(Error::SigningRequired)?;
            let selection = Selection {
                public_key: signer.public_key.clone(),
                fingerprint: signer.fingerprint.clone(),
            };
            persist(&selection_path, &selection)?;
            selection
        };
        let path = self
            .repo
            .join("omni-state/operations")
            .join(format!("{}.signed", intent.prepared.operation_id));
        let raw = if path.exists() {
            let raw = bounded_read(&path, call.limit)?;
            call.account_bytes(raw.len())?;
            raw
        } else {
            let signer = self
                .signer
                .as_ref()
                .filter(|s| {
                    s.public_key == selection.public_key && s.fingerprint == selection.fingerprint
                })
                .ok_or(Error::SigningRequired)?;
            let payload = self.commit_payload(intent);
            let key_path = scratch.join("selected-public.pub");
            fs::write(&key_path, &signer.public_key)?;
            let signed = tool(
                &self.runtime,
                scratch,
                "/usr/bin/ssh-keygen",
                &[
                    "-Y",
                    "sign",
                    "-n",
                    "git",
                    "-f",
                    key_path.to_str().ok_or(Error::Invalid)?,
                ],
                &payload,
                Some(&signer.socket),
                call,
            )?;
            if !signed.status.success() {
                return Err(Error::SigningRequired);
            }
            let signature = std::str::from_utf8(&signed.stdout).map_err(|_| Error::Recovery)?;
            let raw = insert_signature(&payload, signature)?;
            verify_expected(&self.runtime, &raw, &payload, &selection, scratch, call)?;
            let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or(Error::Invalid)?)?;
            file.write_all(&raw)?;
            file.as_file().sync_all()?;
            file.persist_noclobber(&path).map_err(|_| Error::Io)?;
            File::open(path.parent().ok_or(Error::Invalid)?)?.sync_all()?;
            raw
        };
        verify_expected(
            &self.runtime,
            &raw,
            &self.commit_payload(intent),
            &selection,
            scratch,
            call,
        )?;
        oid_output(self.git(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            &raw,
            scratch,
            None,
            call,
        )?)
    }

    /// Verify the exact original public selected signer/payload; provider absence never blocks immutable inspection.
    pub(super) fn verify_signed_candidate(
        &self,
        intent: &Intent,
        actual: &[u8],
        scratch: &Path,
        call: &Call,
    ) -> Result<(), Error> {
        let selection: Selection = load(
            &self
                .repo
                .join("omni-state/operations")
                .join(format!("{}.signer.json", intent.prepared.operation_id)),
        )?;
        verify_expected(
            &self.runtime,
            actual,
            &self.commit_payload(intent),
            &selection,
            scratch,
            call,
        )
    }
}

/// Strict public-key material only; private-key PEM/OpenSSH containers never reach a tool or persisted metadata.
fn public_material(value: &str) -> Result<String, Error> {
    if value.len() > 4096 || value.trim().contains(['\n', '\r']) {
        return Err(Error::Invalid);
    }
    let mut fields = value.split_whitespace();
    let kind = fields.next().ok_or(Error::Invalid)?;
    let data = fields.next().ok_or(Error::Invalid)?;
    if !matches!(
        kind,
        "ssh-ed25519"
            | "ssh-rsa"
            | "ecdsa-sha2-nistp256"
            | "ecdsa-sha2-nistp384"
            | "ecdsa-sha2-nistp521"
            | "sk-ssh-ed25519@openssh.com"
            | "sk-ecdsa-sha2-nistp256@openssh.com"
    ) || data.is_empty()
        || !data
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"+/=".contains(&v))
    {
        return Err(Error::Invalid);
    }
    Ok(format!("{kind} {data}\n"))
}
fn validate_identity(value: &GitIdentity) -> Result<(), Error> {
    for value in [&value.name, &value.email] {
        if value.trim().is_empty()
            || value.len() > 256
            || value
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>'))
        {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}

/// Fixed system tools, cleared environment, explicitly selected socket; never a shell/provider callback.
fn tool(
    runtime: &native::Runtime,
    scratch: &Path,
    program: &str,
    args: &[&str],
    input: &[u8],
    socket: Option<&Path>,
    call: &Call,
) -> Result<native::Output, Error> {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("TMPDIR", scratch)
        .args(args)
        .current_dir(scratch);
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    }
    let output = runtime.execute(command, input, call)?;
    if output.status.signal().is_some() {
        return Err(Error::Git);
    }
    Ok(output)
}

/// Remove only the actual Git gpgsig header, preserving all other exact signed payload bytes.
fn signature_parts(raw: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), Error> {
    let end = raw
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or(Error::Recovery)?
        + 1;
    let mut payload = Vec::with_capacity(raw.len());
    let mut signature = Vec::new();
    let mut in_signature = false;
    let mut found = false;
    for line in raw[..end].split_inclusive(|b| *b == b'\n') {
        if let Some(value) = line.strip_prefix(b"gpgsig ") {
            if found {
                return Err(Error::Recovery);
            }
            found = true;
            in_signature = true;
            signature.extend_from_slice(value);
        } else if in_signature && line.starts_with(b" ") {
            signature.extend_from_slice(&line[1..]);
        } else {
            in_signature = false;
            payload.extend_from_slice(line);
        }
    }
    payload.extend_from_slice(&raw[end..]);
    Ok((payload, found.then_some(signature)))
}
fn insert_signature(payload: &[u8], signature: &str) -> Result<Vec<u8>, Error> {
    if !signature.starts_with("-----BEGIN SSH SIGNATURE-----\n")
        || !signature
            .trim_end()
            .ends_with("-----END SSH SIGNATURE-----")
    {
        return Err(Error::Recovery);
    }
    let end = payload
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or(Error::Recovery)?
        + 1;
    let mut result = Vec::with_capacity(payload.len() + signature.len() + 64);
    result.extend_from_slice(&payload[..end]);
    for (index, line) in signature.lines().enumerate() {
        result.extend_from_slice(if index == 0 { b"gpgsig " } else { b" " });
        result.extend_from_slice(line.as_bytes());
        result.push(b'\n');
    }
    result.extend_from_slice(&payload[end..]);
    if result.len() > CONTROL_LIMIT {
        return Err(Error::Capacity);
    }
    Ok(result)
}
fn verify_expected(
    runtime: &native::Runtime,
    actual: &[u8],
    expected: &[u8],
    selection: &Selection,
    scratch: &Path,
    call: &Call,
) -> Result<(), Error> {
    if signature_parts(actual)?.0 != expected {
        return Err(Error::Recovery);
    }
    let known = KnownSigner::selected("omni-native-selected", &selection.public_key)?;
    if verify(runtime, actual, &[known], scratch, call)?.state != SignatureState::ValidKnown {
        return Err(Error::Recovery);
    }
    Ok(())
}
fn verify(
    runtime: &native::Runtime,
    raw: &[u8],
    known: &[KnownSigner],
    scratch: &Path,
    call: &Call,
) -> Result<Verification, Error> {
    if known.len() as u64 > call.files {
        return Err(Error::Capacity);
    }
    let (payload, signature) = signature_parts(raw)?;
    let Some(signature) = signature else {
        return Ok(Verification {
            state: SignatureState::Unsigned,
            principal: None,
            fingerprint: None,
        });
    };
    if !signature.starts_with(b"-----BEGIN SSH SIGNATURE-----\n") {
        return Err(Error::Invalid);
    }
    let signature_path = scratch.join("verification.sig");
    fs::write(&signature_path, &signature)?;
    let signature_path = signature_path.to_str().ok_or(Error::Invalid)?;
    let checked = tool(
        runtime,
        scratch,
        "/usr/bin/ssh-keygen",
        &["-Y", "check-novalidate", "-n", "git", "-s", signature_path],
        &payload,
        None,
        call,
    )?;
    if !checked.status.success() {
        return Ok(Verification {
            state: SignatureState::Invalid,
            principal: None,
            fingerprint: None,
        });
    }
    let fingerprint = std::str::from_utf8(&checked.stdout)
        .ok()
        .and_then(|s| s.split_whitespace().find(|v| v.starts_with("SHA256:")))
        .map(str::to_owned);
    for signer in known {
        call.account_files(1)?;
        let allowed = scratch.join("native-allowed-signers");
        fs::write(
            &allowed,
            format!(
                "{} namespaces=\"git\" {}",
                signer.principal, signer.public_key
            ),
        )?;
        let verified = tool(
            runtime,
            scratch,
            "/usr/bin/ssh-keygen",
            &[
                "-Y",
                "verify",
                "-f",
                allowed.to_str().ok_or(Error::Invalid)?,
                "-I",
                &signer.principal,
                "-n",
                "git",
                "-s",
                signature_path,
            ],
            &payload,
            None,
            call,
        )?;
        if verified.status.success() {
            return Ok(Verification {
                state: SignatureState::ValidKnown,
                principal: Some(signer.principal.clone()),
                fingerprint,
            });
        }
    }
    Ok(Verification {
        state: SignatureState::ValidUnknown,
        principal: None,
        fingerprint,
    })
}
