//! Descriptor-qualified retirement using the existing Refresh writer and metadata loan.
use super::*;

/// Observed outcomes, never retirement authority or a reader-settlement receipt.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeCorpusRetirement {
    pub retired: u64,
    pub pinned: u64,
    pub retained_unknown: u64,
}

fn open(parent: &File, name: &str) -> Result<File> {
    Ok(File::from(rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?))
}
fn identity(file: &File) -> Result<(u64, u64)> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}
struct Controls<'a> {
    namespace: &'a File,
    name: &'a str,
    directory: &'a File,
    files: [(&'static str, &'a File, [u64; 8]); 3],
}
impl Controls<'_> {
    fn check(&self, removed: usize) -> Result<()> {
        anyhow::ensure!(
            identity(&open(self.namespace, self.name)?)? == identity(self.directory)?,
            "retirement generation name replaced"
        );
        for (name, held, version) in &self.files[removed..] {
            anyhow::ensure!(
                stamp(held)? == *version && stamp(&open(self.directory, name)?)? == *version,
                "retirement controls changed; actual owner retained"
            );
        }
        Ok(())
    }
}
fn member(members: &File, name: &str, owner: &NativeCorpusDraft) -> Result<tuple::Member> {
    let mut offset = 0;
    while offset < members.metadata()?.len() {
        owner.check()?;
        let (row, next) = read_frame::<tuple::Member>(members, offset)?;
        if row.name == name {
            return Ok(row);
        }
        offset = next;
    }
    bail!("unknown generated member; preserve entire generation")
}
// Replay one bounded manifest frame at a time instead of allocating an all-members table.
fn preflight(
    directory: &File,
    members: &File,
    prefix: &str,
    depth: usize,
    owner: &NativeCorpusDraft,
    state: &selection::Selection,
) -> Result<u64> {
    anyhow::ensure!(depth <= 32, "unexpected generated hierarchy");
    let before = stamp(directory)?;
    let mut count = 0;
    for entry in rustix::fs::Dir::read_from(directory)? {
        owner.check()?;
        state.retirement_context(owner)?;
        let entry = entry?;
        let raw = entry.file_name().to_bytes();
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(raw)?;
        if prefix.is_empty() && matches!(name, "tuple.json" | "members.rows" | "READERS") {
            continue;
        }
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        let row = member(members, &path, owner)?;
        let file = open(directory, name)?;
        let meta = file.metadata()?;
        anyhow::ensure!(
            (meta.is_dir() || meta.is_file())
                && meta.is_dir() == row.directory
                && stamp(&file)? == row.version,
            "changed generated member"
        );
        if meta.is_file() {
            anyhow::ensure!(
                row.sha256.as_deref() == Some(tuple::hash(&file, || owner.check())?.as_str()),
                "changed generated content"
            );
        } else {
            anyhow::ensure!(
                row.sha256.is_none(),
                "directory has unexpected hash payload"
            );
            drop(row);
            count += preflight(&file, members, &path, depth + 1, owner, state)?;
        }
        count += 1;
    }
    anyhow::ensure!(
        stamp(directory)? == before,
        "generation changed during retirement preflight"
    );
    Ok(count)
}
fn remove_members(
    directory: &File,
    members: &File,
    prefix: &str,
    depth: usize,
    owner: &NativeCorpusDraft,
    state: &selection::Selection,
    controls: &Controls<'_>,
) -> Result<()> {
    anyhow::ensure!(depth <= 32, "unexpected generated hierarchy");
    // Restart after each removal: directory stream offsets are not stable across unlink.
    loop {
        let mut removed = false;
        for entry in rustix::fs::Dir::read_from(directory)? {
            owner.check()?;
            state.retirement_context(owner)?;
            controls.check(0)?;
            let entry = entry?;
            let raw = entry.file_name().to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            let name = std::str::from_utf8(raw)?;
            if prefix.is_empty() && matches!(name, "tuple.json" | "members.rows" | "READERS") {
                continue;
            }
            let path = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            let row = member(members, &path, owner)?;
            let held = open(directory, name)?;
            anyhow::ensure!(
                identity(&held)? == (row.version[0], row.version[1]),
                "retirement member replaced"
            );
            let is_directory = row.directory;
            let version = row.version;
            drop(row);
            if is_directory {
                remove_members(&held, members, &path, depth + 1, owner, state, controls)?;
            } else {
                anyhow::ensure!(stamp(&held)? == version, "retirement file changed");
            }
            owner.check()?;
            state.retirement_context(owner)?;
            controls.check(0)?;
            anyhow::ensure!(
                identity(&open(directory, name)?)? == identity(&held)?,
                "retirement name replaced"
            );
            rustix::fs::unlinkat(
                directory,
                name,
                if is_directory {
                    rustix::fs::AtFlags::REMOVEDIR
                } else {
                    rustix::fs::AtFlags::empty()
                },
            )?;
            removed = true;
            break;
        }
        if !removed {
            break;
        }
    }
    directory.sync_all()?;
    Ok(())
}
pub(super) fn retire(
    state: &mut selection::Selection,
    owner: &mut NativeCorpusDraft,
    _progress: &mut impl FnMut(NativeCorpusProgress),
) -> Result<NativeCorpusRetirement> {
    let (namespace, selected) = state.retirement_context(owner)?;
    // This fixed peak covers header, a manifest frame with decoded strings, hash window,
    // and the bounded 32-level directory stack. It is already held by this draft's Work.
    anyhow::ensure!(
        owner
            .work
            .as_ref()
            .context("actual metadata loan")?
            .remaining()?
            .memory_bytes
            >= METADATA,
        "retirement metadata is not prepaid"
    );
    type Inspected = (File, File, File, File, [[u64; 8]; 3]);
    let mut report = NativeCorpusRetirement::default();
    for entry in rustix::fs::Dir::read_from(&namespace)? {
        owner.check()?;
        state.retirement_context(owner)?;
        let entry = entry?;
        let Ok(name) = std::str::from_utf8(entry.file_name().to_bytes()) else {
            continue;
        };
        if !name.starts_with("native-corpus-") || name == selected {
            continue;
        }
        let inspected = (|| -> Result<Option<Inspected>> {
            let directory = open(&namespace, name)?;
            anyhow::ensure!(directory.metadata()?.is_dir(), "unowned generated entry");
            let readers = open(&directory, "READERS")?;
            anyhow::ensure!(readers.metadata()?.is_file(), "invalid generation pin");
            match fs2::FileExt::try_lock_exclusive(&readers) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) => return Err(e.into()),
            }
            let header = open(&directory, "tuple.json")?;
            let header_version = stamp(&header)?;
            let readers_version = stamp(&readers)?;
            let length = header_version[2];
            anyhow::ensure!(
                header.metadata()?.is_file() && length <= FRAME as u64,
                "invalid generation header"
            );
            let mut bytes = vec![0; length as usize];
            header.read_exact_at(&mut bytes, 0)?;
            let data: tuple::Header = serde_json::from_slice(&bytes)?;
            anyhow::ensure!(
                data.format == 1
                    && data.library == owner.job.cause.library.as_str()
                    && data.original_root == owner.job.cause.inventory.owner.root
                    && data.stage_root == identity(&directory)?,
                "generation is not owned by selected library"
            );
            let members = open(&directory, "members.rows")?;
            let members_version = stamp(&members)?;
            anyhow::ensure!(
                members.metadata()?.is_file()
                    && members.metadata()?.len() == data.members_length
                    && tuple::hash(&members, || owner.check())? == data.members_sha256,
                "invalid member inventory"
            );
            let versions = [header_version, members_version, readers_version];
            anyhow::ensure!(
                preflight(&directory, &members, "", 0, owner, state)? == data.members,
                "incomplete generation inventory"
            );
            Ok(Some((directory, readers, header, members, versions)))
        })();
        match inspected {
            Ok(None) => report.pinned += 1,
            Err(_) => {
                owner.check()?;
                report.retained_unknown += 1;
            }
            Ok(Some((directory, readers, header, members, versions))) => {
                #[cfg(test)]
                if let Some(hook) = owner.test_before_retirement_delete.take() {
                    hook();
                }
                state.retirement_context(owner)?;
                anyhow::ensure!(
                    identity(&open(&namespace, name)?)? == identity(&directory)?,
                    "generation name replaced"
                );
                let controls = Controls {
                    namespace: &namespace,
                    name,
                    directory: &directory,
                    files: [
                        ("tuple.json", &header, versions[0]),
                        ("members.rows", &members, versions[1]),
                        ("READERS", &readers, versions[2]),
                    ],
                };
                controls.check(0)?;
                remove_members(&directory, &members, "", 0, owner, state, &controls)?;
                for (removed, (control, held, _)) in controls.files.iter().enumerate() {
                    owner.check()?;
                    state.retirement_context(owner)?;
                    controls.check(removed)?;
                    anyhow::ensure!(
                        identity(&open(&directory, control)?)? == identity(held)?,
                        "retirement control replaced"
                    );
                    rustix::fs::unlinkat(&directory, *control, rustix::fs::AtFlags::empty())?;
                }
                directory.sync_all()?;
                state.retirement_context(owner)?;
                controls.check(3)?;
                rustix::fs::unlinkat(&namespace, name, rustix::fs::AtFlags::REMOVEDIR)?;
                namespace.sync_all()?;
                report.retired += 1;
            }
        }
    }
    Ok(report)
}
