//! Private generations, bounded recovery, and native-handle owner delegation.
use super::*;
use std::sync::Arc;

/// Private authority is not reconstructed from mutable public display paths.
#[derive(Debug)]
pub(crate) struct StageAuthority {
    root: File,
    operation: String,
    snapshot: ContentDigest,
    conflicts: Vec<RelativePath>,
}

/// Verify an archive into a new private generation, preserving all existing evidence.
pub fn stage(
    archive: &mut File,
    work: &mut Work<'_>,
    private_dir: &Path,
    operation_id_value: &str,
    existing: &[RelativePath],
) -> Result<Staged, Error> {
    operation_id(operation_id_value)?;
    gate(work)?;
    let parent = native::bind(private_dir, work)?;
    let generation = private_dir.join(operation_id_value);
    let prior = native::directory(&parent, Path::new(operation_id_value), false, work);
    if let Ok(root) = &prior {
        if native::exists(root, "INCOMPLETE", work)? {
            return Err(Error::Interrupted);
        }
        if !native::exists(root, "COMPLETE", work)? {
            return Err(Error::Conflict);
        }
    } else if let Err(error) = &prior
        && *error != Error::Io
    {
        return Err(*error);
    }
    let opened = open_verified(archive, work)?;
    if !opened.installable {
        return Err(Error::Unsupported);
    }
    if let Ok(root) = prior {
        let manifest = recover(&root, &opened.snapshot_id, operation_id_value, work)?;
        if manifest.entries != opened.manifest.entries {
            return Err(Error::Conflict);
        }
        for entry in &manifest.entries {
            source::open(&root, &staged_name(entry), &entry.sha256, work)?;
        }
        return build(
            root,
            &generation,
            operation_id_value,
            opened.snapshot_id,
            &manifest,
            existing,
            work,
        );
    }
    note_file(work)?;
    rustix::fs::mkdirat(
        &parent,
        operation_id_value,
        rustix::fs::Mode::from_raw_mode(0o700),
    )
    .map_err(|_| Error::Conflict)?;
    let root = native::directory(&parent, Path::new(operation_id_value), false, work)?;
    native::file(&root, Path::new("INCOMPLETE"), true, work)?
        .sync_all()
        .map_err(|_| Error::Io)?;
    for entry in &opened.manifest.entries {
        let member = &opened.members[opened
            .members
            .binary_search_by(|m| m.name.as_str().cmp(entry.path.as_str()))
            .map_err(|_| Error::Disagreement)?];
        let mut output = native::file(&root, &staged_name(entry), true, work)?;
        let digest = stream(archive, member, work, &mut Out::Disk(&mut output))?;
        if digest != entry.sha256.as_str() {
            return Err(Error::Checksum);
        }
        output.sync_all().map_err(|_| Error::Io)?;
    }
    let mut manifest_file = native::file(&root, Path::new("MANIFEST"), true, work)?;
    write_bytes(&mut manifest_file, &opened.canonical, work)?;
    manifest_file.sync_all().map_err(|_| Error::Io)?;
    let mut plan = native::file(&root, Path::new("PLAN"), true, work)?;
    plan_parts(
        &opened.snapshot_id,
        operation_id_value,
        &opened.manifest,
        |part| write_bytes(&mut plan, part, work),
    )?;
    plan.sync_all().map_err(|_| Error::Io)?;
    native::file(&root, Path::new("COMPLETE"), true, work)?
        .sync_all()
        .map_err(|_| Error::Io)?;
    root.sync_all().map_err(|_| Error::Io)?;
    rustix::fs::unlinkat(&root, "INCOMPLETE", rustix::fs::AtFlags::empty())
        .map_err(|_| Error::Io)?;
    root.sync_all().map_err(|_| Error::Io)?;
    build(
        root,
        &generation,
        operation_id_value,
        opened.snapshot_id,
        &opened.manifest,
        existing,
        work,
    )
}

/// Delegate only freshly recovered inventory and the same verified native source descriptor.
/// `work` also bounds recovery, source hashing, and the owner's private verified copy.
pub fn apply(
    staged: &Staged,
    install: &mut dyn SafeApply,
    git: &mut dyn GitTransactions,
    work: &mut Work<'_>,
) -> Result<Applied, Error> {
    let authority = &staged.authority;
    let mut applied = Vec::new();
    charge_fresh(
        work,
        authority
            .conflicts
            .iter()
            .map(|p| p.as_str().len() as u64 + 64)
            .sum(),
        0,
    )?;
    let mut conflicts = authority.conflicts.clone();
    if !native::exists(&authority.root, "COMPLETE", work)?
        || native::exists(&authority.root, "INCOMPLETE", work)?
    {
        return Ok(Applied {
            state: ApplyState::Interrupted,
            applied,
            conflicts,
        });
    }
    let manifest = recover(
        &authority.root,
        &authority.snapshot,
        &authority.operation,
        work,
    )?;
    if !staged.installable || !manifest.required_features.is_empty() {
        return Ok(Applied {
            state: ApplyState::Refused,
            applied,
            conflicts,
        });
    }
    charge_fresh(work, metadata::manifest(&manifest).bound()?, 0)?;
    for entry in manifest
        .entries
        .iter()
        .filter(|e| matches!(e.role, Role::Core | Role::Original))
        .chain(manifest.entries.iter().filter(|e| e.role == Role::History))
    {
        // Verify even conflicting bytes, as in the original apply contract.
        let mut source =
            match source::open(&authority.root, &staged_name(entry), &entry.sha256, work) {
                Ok(source) => source,
                Err(Error::Checksum | Error::Drift) => {
                    return Ok(Applied {
                        state: ApplyState::Refused,
                        applied,
                        conflicts,
                    });
                }
                Err(error) => return Err(error),
            };
        let _fold = preallocate(
            work,
            (entry.path.as_str().len() * 3
                + conflicts
                    .iter()
                    .map(|p| p.as_str().len() * 3)
                    .sum::<usize>()) as u64,
        )?;
        if conflicts
            .iter()
            .any(|p| p.as_str().to_lowercase() == entry.path.as_str().to_lowercase())
        {
            continue;
        }
        // The owner copy admits its own examination; do not delegate when no unit remains.
        check_file(work)?;
        if entry.role == Role::History {
            if git.admit_inert_bundle(&authority.operation, &mut source, work)? == GitAdmit::Refused
            {
                return Ok(Applied {
                    state: ApplyState::Conflict,
                    applied,
                    conflicts,
                });
            }
            continue;
        }
        let outcome = if entry.role == Role::Core {
            install.apply_text_change(&authority.operation, &entry.path, None, &mut source, work)?
        } else {
            install.install_new(&authority.operation, &entry.path, &mut source, work)?
        };
        match outcome {
            ApplyOutcome::Saved => applied.push(entry.path.clone()),
            ApplyOutcome::Conflict => {
                conflicts.push(entry.path.clone());
                return Ok(Applied {
                    state: ApplyState::Conflict,
                    applied,
                    conflicts,
                });
            }
            ApplyOutcome::Indeterminate => {
                return Ok(Applied {
                    state: ApplyState::Indeterminate,
                    applied,
                    conflicts,
                });
            }
        }
    }
    let state = if conflicts.is_empty() {
        ApplyState::Completed
    } else {
        ApplyState::Conflict
    };
    Ok(Applied {
        state,
        applied,
        conflicts,
    })
}

/// Construct display fields only after funding all semantic/path clones and conflict copies.
fn build(
    root: File,
    generation: &Path,
    operation: &str,
    snapshot: ContentDigest,
    manifest: &Manifest,
    existing: &[RelativePath],
    work: &mut Work<'_>,
) -> Result<Staged, Error> {
    charge_fresh(
        work,
        metadata::manifest(manifest)
            .bound()?
            .checked_add((manifest.entries.len() * generation.as_os_str().len() * 4) as u64)
            .ok_or(Error::TooLarge)?,
        0,
    )?;
    let files = manifest
        .entries
        .iter()
        .map(|entry| StagedFile {
            path: entry.path.clone(),
            role: entry.role,
            digest: entry.sha256.clone(),
            staged: generation.join(staged_name(entry)),
        })
        .collect();
    let conflicts = conflicts_for(&manifest.entries, existing, work)?;
    let authority = Arc::new(StageAuthority {
        root,
        operation: operation.to_owned(),
        snapshot: snapshot.clone(),
        conflicts: conflicts.clone(),
    });
    Ok(attach_roles(Staged {
        operation_id: operation.to_owned(),
        snapshot_id: snapshot,
        generation: generation.to_path_buf(),
        files,
        conflicts,
        history: None,
        journal: None,
        installable: manifest.required_features.is_empty(),
        authority,
    }))
}

/// Recover only the expected snapshot and operation. PLAN bytes never choose paths or roles.
fn recover(
    root: &File,
    snapshot: &ContentDigest,
    operation: &str,
    work: &mut Work<'_>,
) -> Result<Manifest, Error> {
    let mut file = native::file(root, Path::new("MANIFEST"), false, work)?;
    let canonical = read_source(&mut file, work)?;
    if digest_bytes(&canonical)? != *snapshot {
        return Err(Error::Conflict);
    }
    let manifest = manifest::parse(&canonical, work)?;
    let mut plan = native::file(root, Path::new("PLAN"), false, work)?;
    let _scratch = preallocate(work, 8192)?;
    let mut buffer = [0u8; 8192];
    plan_parts(snapshot, operation, &manifest, |part| {
        for expected in part.chunks(buffer.len()) {
            gate(work)?;
            plan.read_exact(&mut buffer[..expected.len()])
                .map_err(|_| Error::Indeterminate)?;
            note_read(work, expected.len() as u64)?;
            if &buffer[..expected.len()] != expected {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    })?;
    gate(work)?;
    let count = plan.read(&mut buffer[..1]).map_err(|_| Error::Io)?;
    note_read(work, count as u64)?;
    if count != 0 {
        return Err(Error::Conflict);
    }
    Ok(manifest)
}

/// Emit bounded plan fragments, preserving the original snapshot-first framing.
fn plan_parts(
    snapshot: &ContentDigest,
    operation: &str,
    manifest: &Manifest,
    mut part: impl FnMut(&[u8]) -> Result<(), Error>,
) -> Result<(), Error> {
    for text in [snapshot.as_str(), operation] {
        part(text.as_bytes())?;
        part(b"\n")?;
    }
    for entry in &manifest.entries {
        for text in [
            entry.path.as_str(),
            role_token(entry.role),
            entry.sha256.as_str(),
        ] {
            part(text.as_bytes())?;
            part(b"\n")?;
        }
    }
    Ok(())
}

/// Charge before each native write and reconcile actual bytes even on a short write.
fn write_bytes(file: &mut File, mut bytes: &[u8], work: &mut Work<'_>) -> Result<(), Error> {
    charge_fresh(work, 0, bytes.len() as u64)?;
    while !bytes.is_empty() {
        gate(work)?;
        let n = file.write(bytes).map_err(|_| Error::Io)?;
        if n == 0 {
            return Err(Error::Io);
        }
        note_written(work, n as u64)?;
        bytes = &bytes[n..];
    }
    Ok(())
}
