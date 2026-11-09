//! Canonical annotations created by source import, using existing M1 and bootstrap owners.
use crate::{
    AssetMetadata, KnowledgeBase,
    native_service::{NativeLibraryService, NativeSource},
    storage,
};
use anyhow::{Context, Result, bail};
use knowledge_core::{
    admission::{Account, Decision, Holding, Origin},
    backend::Budget,
    jobs::Reservation,
};
use knowledge_storage::Preflight;
use omni_service::{
    self as wire,
    authority::{LocalPolicy, NativeAuthority, TransportFacts},
};
use std::sync::Arc;

/// Reserve the existing account's complete synchronous inventory/copy scratch before effects.
fn holding(memory_bytes: u64, temporary_bytes: u64) -> Result<Holding> {
    match Account::current()?.reserve(
        Reservation {
            memory_bytes,
            temporary_bytes,
        },
        Origin::BeforeVerification,
    )? {
        Decision::Ready(value) => Ok(value),
        Decision::Waiting(reason) => bail!("import annotation is waiting: {reason:?}"),
    }
}

/// Observe the imported file in the actual M1 journal; path hashes never become source UUIDs.
fn source(kb: &KnowledgeBase, path: &str) -> Result<NativeSource> {
    let (_, owner) = NativeAuthority::new();
    let (service, selection) = NativeLibraryService::ordinary_local_owned(
        kb.clone(),
        LocalPolicy {
            mode: wire::ExposureMode::ReadWrite,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        },
        &owner,
    )?;
    let session = service
        .connect_selected(&selection, TransportFacts::OwnerStdio)
        .map_err(|e| anyhow::anyhow!("import selection refused: {e:?}"))?;
    let command = wire::decode_request(&serde_json::to_vec(&serde_json::json!({
        "wire_version":1, "request_id":"import-annotations", "command":{
            "op":"refresh", "library_id":service.library_id(), "mount_ids":[],
            "verification":"full_hash", "embeddings":false, "operation_id":"import-annotations",
            "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":120000,"max_memory_bytes":"536870912"}
        }
    }))?)?.command;
    let permit = service
        .native_scope(&session, &command, None)
        .map_err(|e| anyhow::anyhow!("import inventory refused: {e:?}"))?;
    let mut scratch = holding(4 * 1024 * 1024, 16 * 1024 * 1024)?;
    let result = (|| {
        let context = holding(2 * 1024 * 1024, 0)?;
        service.reconcile_inventory(
            &owner,
            permit.clone(),
            "pages",
            context,
            &Preflight {
                holding: &scratch,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 64 * 1024 * 1024,
                    max_millis: 120000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
    })();
    scratch.finish()?;
    let (inventory, report) = result?;
    drop(report);
    let mut offset = 0;
    loop {
        let funding = holding(2 * 1024 * 1024 + 128 * 1024, 0)?;
        let page = inventory.page(
            offset,
            Budget {
                max_files: 64,
                max_bytes: 64 * 1024 * 1024,
                max_millis: 120000,
                max_memory_bytes: 2 * 1024 * 1024,
            },
            funding,
        )?;
        let found = page
            .observations()
            .find(|row| row.observation().stat.path.as_str() == path);
        if let Some(observed) = found {
            let source = service.source_for_observed(
                &permit,
                observed,
                Budget {
                    max_files: 1,
                    max_bytes: 64 * 1024 * 1024,
                    max_millis: 120000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            )?;
            drop(page);
            return Ok(source);
        }
        let next = page.next_offset();
        let complete = page.complete();
        let valid = page.issue().is_none();
        let mut funding = page
            .into_funding()
            .map_err(|_| anyhow::anyhow!("import inventory page still borrowed"))?;
        funding.finish()?;
        anyhow::ensure!(valid, "import inventory replay is incomplete");
        anyhow::ensure!(
            !complete && next > offset,
            "imported source absent from actual M1 inventory"
        );
        offset = next;
    }
}

/// Create exactly one visible pointer for a newly copied original.
/// Existing annotations are collisions, never permission to replace an authored pointer or its body.
pub(crate) fn imported(kb: &KnowledgeBase, metadata: &AssetMetadata) -> Result<()> {
    anyhow::ensure!(
        kb.config.version == 2,
        "legacy migration is not yet shipped; version-1 libraries support inventory and preview only"
    );
    if metadata.tags.is_empty() && metadata.description.is_empty() {
        return Ok(());
    }
    let source = Arc::new(source(kb, &metadata.path)?);
    uuid::Uuid::parse_str(source.id().as_str())
        .context("M1 source identity must be an actual UUID")?;
    let pointer = format!("sources/{}.md", source.id().as_str());
    let size = metadata.tags.iter().try_fold(
        metadata.path.capacity() + metadata.description.capacity(),
        |n, tag| {
            n.checked_add(tag.capacity())
                .context("annotation quote overflow")
        },
    )?;
    let mut credit = holding(
        (size as u64)
            .checked_mul(64)
            .and_then(|n| n.checked_add(65536))
            .context("annotation quote overflow")?,
        0,
    )?;
    let result = (|| {
        source.check()?;
        let data = serde_json::json!({
            "mount_id":null, "path":metadata.path, "digest_state":"verified", "digest":source.version().as_str(),
            "size_bytes":source.file()?.metadata()?.len(), "media_type":mime_guess::from_path(&metadata.path).first_or_octet_stream().essence_str(),
            "file_id":null, "availability":"available",
            "title":std::path::Path::new(&metadata.path).file_name().and_then(|s|s.to_str()).unwrap_or(""),
            "description":metadata.description, "tags":metadata.tags
        });
        let text = format!("---\n{}---\n", serde_yaml::to_string(&data)?);
        storage::import_pointer(kb, &pointer, text.as_bytes(), source.clone())
    })();
    credit.finish()?;
    result
}
