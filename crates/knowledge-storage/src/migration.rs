//! Read-only migration inventory: unknown state and conflicting authored trees are never garbage.
use crate::{
    bootstrap::{Bounds, Preflight},
    *,
};
use knowledge_core::manifest::{ManifestDocument, ManifestLimits, ManifestState};
/// Disposition from actual selected filesystem inspection, not permission to convert/delete data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MigrationKind {
    /// Legacy authored sidecar; preserve exact bytes and effective annotation union in any later explicit plan.
    LegacyAnnotation,
    /// Recognized old generated index area; ownership still needs validation before retirement.
    LegacyIndex,
    /// Unknown legacy state; never wholesale .omni deletion.
    UnknownLegacyState,
    /// Existing authored pages tree, never silently merged with wiki.
    Pages,
    /// Existing authored wiki tree, never copied into a second authored authority.
    Wiki,
}
/// One actual metadata-only migration item, no user body/credentials in the inventory.
#[derive(Clone, Debug, Serialize)]
pub struct MigrationItem {
    /// Exact selected relative path.
    pub path: String,
    /// Observed native file size, not a hash/current content proof.
    pub bytes: u64,
    /// Explicit preserve/conflict classification.
    pub kind: MigrationKind,
    /// Nonregular nodes remain unsupported/conflict material, not followed sources.
    pub regular: bool,
}
/// Bounded inventory, never implicit migration approval or a false empty result.
pub struct MigrationInventory {
    /// Actual Core interpretation; legacy/future/invalid markers remain read-only.
    pub manifest: ManifestState,
    /// Retained bounded metadata items.
    pub items: Vec<MigrationItem>,
    /// Both authored trees require explicit owner mapping, not an automatic merge/copy.
    pub conflicting_text_roots: bool,
    /// Relevant selected legacy inventory finished successfully.
    pub complete: bool,
}
impl<P: WritePolicy> Store<P> {
    /// Inventory selected legacy authority/conflicts using actual no-follow native metadata only.
    /// This creates/deletes/converts nothing; subsequent authored conversion needs an explicit safe delta.
    pub fn migration_inventory(
        &self,
        preflight: &Preflight<'_>,
    ) -> Result<MigrationInventory, Error> {
        let b = Bounds::new(preflight)?;
        let c = self.gate()?;
        let marker = native::text(
            &self.root,
            "omni-knowledge.json",
            b.bytes.min(b.memory / 16),
        )?;
        let doc = ManifestDocument::decode(
            marker.as_bytes(),
            ManifestLimits {
                max_bytes: usize::try_from(b.bytes.min(b.memory / 16))
                    .map_err(|_| Error::Budget)?,
            },
        )
        .map_err(|_| Error::Invalid)?;
        let presence = |name| match native::directory(&self.root, name) {
            Ok(_) => Ok(true),
            Err(Error::Missing) => Ok(false),
            Err(e) => Err(e),
        };
        let pages = presence("pages")?;
        let wiki = presence("wiki")?;
        let mut result = MigrationInventory {
            manifest: doc.state(),
            items: vec![],
            conflicting_text_roots: pages && wiki,
            complete: true,
        };
        let mut stack = vec![];
        for name in [".omni", "pages", "wiki"] {
            match native::directory(&self.root, name) {
                Ok(d) => stack.push((name.to_owned(), d)),
                Err(Error::Missing) => {}
                Err(e) => return Err(e),
            }
        }
        let mut examined = 0u64;
        let mut metadata_bytes = 0u64;
        let mut memory = marker.len() as u64 * 16;
        while let Some((prefix, dir)) = stack.pop() {
            b.check()?;
            self.live(&c)?;
            let fd = rustix::fs::openat(
                &dir,
                ".",
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| Error::Io)?;
            for entry in rustix::fs::Dir::read_from(fd).map_err(|_| Error::Io)? {
                b.check()?;
                let entry = entry.map_err(|_| Error::Io)?;
                let name = entry.file_name().to_str().map_err(|_| Error::Unsupported)?;
                if name == "." || name == ".." {
                    continue;
                }
                examined += 1;
                let length = prefix.len() + name.len() + 1;
                memory = memory
                    .checked_add(length as u64 * 4 + 512)
                    .ok_or(Error::Budget)?;
                metadata_bytes = metadata_bytes
                    .checked_add(length as u64)
                    .ok_or(Error::Budget)?;
                if examined > preflight.budget.max_files
                    || memory > b.memory / 2
                    || metadata_bytes > b.bytes
                {
                    result.complete = false;
                    self.live(&c)?;
                    return Ok(result);
                }
                let path = format!("{prefix}/{name}");
                if c.outside_scope
                    .iter()
                    .any(|p| path == *p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
                {
                    continue;
                }
                let stat = rustix::fs::statat(&dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|_| Error::Io)?;
                let kind = if path.starts_with(".omni/metadata/") {
                    MigrationKind::LegacyAnnotation
                } else if path.starts_with(".omni/indexes/") {
                    MigrationKind::LegacyIndex
                } else if path.starts_with(".omni/") {
                    MigrationKind::UnknownLegacyState
                } else if prefix.starts_with("pages") {
                    MigrationKind::Pages
                } else {
                    MigrationKind::Wiki
                };
                let filetype = rustix::fs::FileType::from_raw_mode(stat.st_mode);
                result.items.push(MigrationItem {
                    path: path.clone(),
                    bytes: stat.st_size.try_into().map_err(|_| Error::Invalid)?,
                    kind,
                    regular: filetype == rustix::fs::FileType::RegularFile,
                });
                if filetype == rustix::fs::FileType::Directory {
                    let child = native::directory(&dir, name)?;
                    if native::marker(&child)? {
                        continue;
                    }
                    stack.push((path, child));
                }
            }
        }
        self.live(&c)?;
        Ok(result)
    }
}
