//! Explicit native selection of an existing user Git database, never automatic discovery/relocation.
use super::*;

/// Current native placement observation supplied by M6, not a model/package flag or stored-path grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitPlacement {
    LocalUnsynced,
    Synchronized,
    Unknown,
}
/// Native user's explicit use-existing selection; use History::init for the separate-history choice.
#[derive(Clone, Copy, Debug)]
pub struct ExistingRepository<'a> {
    pub git_dir: &'a Path,
    pub baseline: &'a str,
    pub placement: GitPlacement,
}
/// Bounded validated config version and original selected ancestry; no executable configuration is copied into app metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExistingBinding {
    pub config_digest: String,
    pub initial: Option<Oid>,
}

impl History {
    /// Use actual existing objects/app refs with a private index, preserving user HEAD/branches/index/tracked media.
    /// Native M6 must establish current ownership/permission and local non-sync placement; these Rust inputs are not wire grants.
    pub fn use_existing(
        selection: ExistingRepository<'_>,
        home: &Path,
        text_root: &str,
        owner: OwnerContext,
        claim: &Claim,
    ) -> Result<Self, Error> {
        if selection.placement != GitPlacement::LocalUnsynced || owner.agent.is_some() {
            return Err(Error::ContextMismatch);
        }
        validate_owner(&owner)?;
        validate_root(text_root)?;
        if !selection.baseline.starts_with("refs/heads/") || selection.baseline.len() > 4096 {
            return Err(Error::Invalid);
        }
        let repo = selected_directory(selection.git_dir)?;
        let home = selected_directory(home)?;
        if home.starts_with(&repo) {
            return Err(Error::Invalid);
        }
        for name in ["config", "HEAD", "packed-refs"] {
            if let Ok(meta) = fs::symlink_metadata(repo.join(name))
                && (!meta.is_file() || meta.file_type().is_symlink())
            {
                return Err(Error::Invalid);
            }
        }
        // No outside alternate/worktree config is followed before native selection/validation.
        for name in [
            "objects/info/alternates",
            "objects/info/http-alternates",
            "commondir",
            "config.worktree",
            "shallow",
        ] {
            if fs::symlink_metadata(repo.join(name)).is_ok() {
                return Err(Error::Invalid);
            }
        }
        let call = Call::new(claim)?;
        let runtime = native::Runtime::default();
        let digest = inspect_configuration(&repo, &runtime, &call)?;
        let scratch = native::Scratch::new(&std::env::temp_dir(), &call)?;
        run(
            &repo,
            scratch.path(),
            &["check-ref-format", selection.baseline],
            b"",
            None,
            &call,
            (None, &runtime),
        )?;
        let format = run(
            &repo,
            scratch.path(),
            &["rev-parse", "--show-object-format"],
            b"",
            None,
            &call,
            (None, &runtime),
        )?;
        if format != b"sha1\n" {
            return Err(Error::Invalid);
        }
        let initial = actual_ref(&repo, selection.baseline, scratch.path(), &runtime, &call)?;
        let state = repo.join("omni-state");
        if fs::symlink_metadata(&state).is_ok() {
            selected_directory(&state)?;
        }
        let binding_path = state.join("binding.json");
        let existing = if binding_path.exists() {
            let previous: Binding = load(&binding_path)?;
            if previous.repo != identity(&repo)?
                || previous.home != identity(&home)?
                || previous.name != owner.name
                || previous.email != owner.email
                || previous.mode != owner.mode
                || previous.text_root != text_root
                || previous.existing.is_none()
            {
                return Err(Error::ContextMismatch);
            }
            // Re-selecting native configuration never rewinds an already active app history to today's user HEAD.
            ExistingBinding {
                config_digest: digest,
                initial: previous.existing.ok_or(Error::Recovery)?.initial,
            }
        } else {
            if state.exists()
                || actual_ref(&repo, ACCEPTED, scratch.path(), &runtime, &call)?.is_some()
            {
                return Err(Error::Conflict);
            }
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&state)?;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(state.join("operations"))?;
            ExistingBinding {
                config_digest: digest,
                initial: initial.clone(),
            }
        };
        drop(scratch);
        let binding = Binding {
            version: 1,
            repo: identity(&repo)?,
            home: identity(&home)?,
            text_root: text_root.to_owned(),
            name: owner.name.clone(),
            email: owner.email.clone(),
            mode: owner.mode.clone(),
            existing: Some(existing),
        };
        let history = Self {
            repo,
            home,
            binding,
            owner,
            runtime,
            signer: None,
            committer: None,
            quiet: Default::default(),
        };
        let lock = history.lock(&call)?;
        if binding_path.exists() {
            let mut file = tempfile::NamedTempFile::new_in(&state)?;
            file.write_all(&serde_json::to_vec(&history.binding).map_err(|_| Error::Recovery)?)?;
            file.as_file().sync_all()?;
            file.persist(&binding_path).map_err(|_| Error::Io)?;
            File::open(&state)?.sync_all()?;
        } else {
            persist(&binding_path, &history.binding)?;
        }
        let scratch = history.scratch(&call)?;
        if history
            .reference(ACCEPTED, scratch.path(), &call)?
            .is_none()
            && let Some(base) = &history
                .binding
                .existing
                .as_ref()
                .ok_or(Error::Recovery)?
                .initial
        {
            if initial.as_ref() != Some(base) {
                return Err(Error::Conflict);
            }
            history.base_tree(Some(base), scratch.path(), &call)?;
            let transaction = format!(
                "start\nverify {} {}\ncreate {ACCEPTED} {}\nprepare\ncommit\n",
                selection.baseline,
                base.as_str(),
                base.as_str()
            );
            history.git(
                &["update-ref", "--stdin", "--no-deref"],
                transaction.as_bytes(),
                scratch.path(),
                None,
                &call,
            )?;
        }
        history.validate()?;
        drop(scratch);
        drop(lock);
        Ok(history)
    }

    /// Actual mode of one selected immutable file; no symlink/submodule is reinterpreted as editable text.
    pub(super) fn selected_mode(
        &self,
        tree: &Oid,
        path: &str,
        scratch: &Path,
        call: &Call,
    ) -> Result<Option<&'static str>, Error> {
        let listing = self.git(
            &["ls-tree", "-z", tree.as_str(), "--", path],
            b"",
            scratch,
            None,
            call,
        )?;
        if listing.is_empty() {
            return Ok(None);
        }
        let text = std::str::from_utf8(&listing).map_err(|_| Error::Invalid)?;
        let (meta, actual) = text
            .strip_suffix('\0')
            .and_then(|s| s.split_once('\t'))
            .ok_or(Error::Invalid)?;
        if actual != path {
            return Err(Error::Invalid);
        }
        if meta.starts_with("100644 blob ") {
            Ok(Some("100644"))
        } else if meta.starts_with("100755 blob ") {
            Ok(Some("100755"))
        } else {
            Err(Error::Invalid)
        }
    }

    /// Existing tracked binary/media may remain unchanged; a text delta cannot silently replace/delete its actual bytes.
    pub(super) fn editable_base(
        &self,
        base: &Oid,
        path: &str,
        scratch: &Path,
        call: &Call,
    ) -> Result<&'static str, Error> {
        let Some(mode) = self.selected_mode(base, path, scratch, call)? else {
            return Ok("100644");
        };
        let spec = format!("{}:{path}", base.as_str());
        let bytes = self.git(&["cat-file", "blob", &spec], b"", scratch, None, call)?;
        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
            return Err(Error::Invalid);
        }
        Ok(mode)
    }
}

/// Read exactly a qualified selected ref, without copying user branch names or moving their HEAD.
fn actual_ref(
    repo: &Path,
    name: &str,
    scratch: &Path,
    runtime: &native::Runtime,
    call: &Call,
) -> Result<Option<Oid>, Error> {
    let output = run(
        repo,
        scratch,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(symref)",
            "--count=2",
            name,
        ],
        b"",
        None,
        call,
        (None, runtime),
    )?;
    let text = std::str::from_utf8(&output).map_err(|_| Error::Recovery)?;
    if text.is_empty() {
        return Ok(None);
    }
    let mut lines = text.lines();
    let fields: Vec<_> = lines.next().ok_or(Error::Recovery)?.split(' ').collect();
    if fields.len() != 3 || fields[0] != name || !fields[2].is_empty() || lines.next().is_some() {
        return Err(Error::Invalid);
    }
    Ok(Some(Oid::parse(fields[1])?))
}

/// Existing config is parsed by native Git with explicit no-includes and stdin: no private config copy or executable import.
fn inspect_configuration(
    repo: &Path,
    runtime: &native::Runtime,
    call: &Call,
) -> Result<String, Error> {
    let config = repo.join("config");
    let before = fs::symlink_metadata(&config)?;
    let bytes = bounded_read(&config, call.limit)?;
    call.account_bytes(bytes.len())?;
    let after = fs::symlink_metadata(&config)?;
    if (
        before.dev(),
        before.ino(),
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) != (
        after.dev(),
        after.ino(),
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) {
        return Err(Error::ContextMismatch);
    }
    let scratch = native::Scratch::new(&std::env::temp_dir(), call)?;
    let parser = scratch.path().join("parser.git");
    fs::create_dir(&parser)?;
    run(
        &parser,
        scratch.path(),
        &["init", "--bare", "--object-format=sha1", "--template=", "."],
        b"",
        None,
        call,
        (None, runtime),
    )?;
    fs::write(parser.join("config"), CONFIG)?;
    let keys = run(
        &parser,
        scratch.path(),
        &[
            "config",
            "--no-includes",
            "--file",
            "-",
            "--null",
            "--name-only",
            "--list",
        ],
        &bytes,
        None,
        call,
        (None, runtime),
    )?;
    for key in keys.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        call.account_files(1)?;
        let key = std::str::from_utf8(key)
            .map_err(|_| Error::Invalid)?
            .to_ascii_lowercase();
        // Includes widen file access before CLI overrides; rewrite/protocol/transport settings widen selected remote/effect.
        if key.starts_with("include.")
            || key.starts_with("includeif.")
            || key.starts_with("protocol.")
            || key.starts_with("url.")
            || key.starts_with("http.")
            || key.starts_with("https.")
            || key == "extensions.worktreeconfig"
            || key == "core.sshcommand"
            || key == "core.gitproxy"
            || key == "core.alternaterefscommand"
        {
            return Err(Error::Invalid);
        }
    }
    let value = digest(&bytes);
    if digest(&bounded_read(&config, call.limit)?) != value {
        return Err(Error::ContextMismatch);
    }
    Ok(value)
}
