// Explicit retained v1 component fixtures; never a native product-startup qualification.
use super::fixture_library::KnowledgeBase;
use std::fs;
use std::path::Path;

/// Construct the old writer/layout explicitly after obtaining the current initializer's seed bytes.
/// V1 metadata/index component tests retain their original assertions and provider settings.
pub fn legacy_init(path: &Path, name: &str) -> anyhow::Result<KnowledgeBase> {
    let mut kb = KnowledgeBase::init(path, name)?;
    fs::rename(kb.root.join("pages"), kb.root.join("wiki"))?;
    fn rewrite(path: &Path) -> anyhow::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if entry.file_name() == "wiki" {
                    rewrite(&entry.path())?;
                }
            } else if entry.path().extension().is_some_and(|ext| ext == "md") {
                let text = fs::read_to_string(entry.path())?;
                fs::write(entry.path(), text.replace("pages/", "wiki/"))?;
            }
        }
        Ok(())
    }
    rewrite(&kb.root)?;
    fs::create_dir_all(kb.root.join(".omni/metadata"))?;
    kb.config.version = 1;
    kb.config.embedding = Some(Default::default());
    fs::write(
        kb.root.join("omni-knowledge.json"),
        serde_json::to_vec_pretty(&kb.config)?,
    )?;
    Ok(kb)
}
