//! MCP tool names taken from the frozen command enum.
//!
//! Owner-only names are omitted from the catalog. That is the published MCP
//! surface, not a decision that the caller is the owner and not a second schema.

use omni_service::Command;

/// Every frozen command name, in enum order.
pub const COMMANDS: &[&str] = &[
    "library_list",
    "library_open",
    "library_create",
    "library_clone",
    "library_status",
    "mount_attach",
    "mount_detach",
    "mount_list",
    "browse",
    "source_details",
    "verify_source",
    "read_range",
    "read_representation",
    "annotate",
    "edit_text",
    "import_file",
    "transfer",
    "trash",
    "hydrate",
    "refresh",
    "pin",
    "process",
    "query",
    "job_status",
    "cancel",
    "operation_status",
    "history_list",
    "history_diff",
    "proposal_list",
    "proposal_diff",
    "confirm_merge",
    "sync",
    "sync_schedule",
    "epub_plan",
    "epub_export",
    "snapshot_plan",
    "snapshot_create",
    "package_inspect",
    "open_frozen",
    "restore_plan",
    "restore_apply",
    "migration_plan",
    "migration_apply",
    "v1_import_plan",
    "v1_export_plan",
    "exposure_get",
    "exposure_set",
];

/// Names the service owner keeps out of the MCP catalog, including in read-write mode.
pub const MCP_EXCLUDED: &[&str] = &[
    "library_open",
    "library_create",
    "library_clone",
    "mount_attach",
    "mount_detach",
    "trash",
    "hydrate",
    "confirm_merge",
    "sync",
    "sync_schedule",
    "epub_plan",
    "epub_export",
    "snapshot_plan",
    "snapshot_create",
    "package_inspect",
    "open_frozen",
    "restore_plan",
    "restore_apply",
    "migration_plan",
    "migration_apply",
    "v1_import_plan",
    "v1_export_plan",
    "exposure_get",
    "exposure_set",
];

/// Return the codec's `op` string for this command.
pub fn command_op(command: &Command) -> &'static str {
    match command {
        Command::LibraryList { .. } => "library_list",
        Command::LibraryOpen { .. } => "library_open",
        Command::LibraryCreate { .. } => "library_create",
        Command::LibraryClone { .. } => "library_clone",
        Command::LibraryStatus { .. } => "library_status",
        Command::MountAttach { .. } => "mount_attach",
        Command::MountDetach { .. } => "mount_detach",
        Command::MountList { .. } => "mount_list",
        Command::Browse { .. } => "browse",
        Command::SourceDetails { .. } => "source_details",
        Command::VerifySource { .. } => "verify_source",
        Command::ReadRange { .. } => "read_range",
        Command::ReadRepresentation { .. } => "read_representation",
        Command::Annotate { .. } => "annotate",
        Command::EditText { .. } => "edit_text",
        Command::ImportFile { .. } => "import_file",
        Command::Transfer { .. } => "transfer",
        Command::Trash { .. } => "trash",
        Command::Hydrate { .. } => "hydrate",
        Command::Refresh { .. } => "refresh",
        Command::Pin { .. } => "pin",
        Command::Process { .. } => "process",
        Command::Query { .. } => "query",
        Command::JobStatus { .. } => "job_status",
        Command::Cancel { .. } => "cancel",
        Command::OperationStatus { .. } => "operation_status",
        Command::HistoryList { .. } => "history_list",
        Command::HistoryDiff { .. } => "history_diff",
        Command::ProposalList { .. } => "proposal_list",
        Command::ProposalDiff { .. } => "proposal_diff",
        Command::ConfirmMerge { .. } => "confirm_merge",
        Command::Sync { .. } => "sync",
        Command::SyncSchedule { .. } => "sync_schedule",
        Command::EpubPlan { .. } => "epub_plan",
        Command::EpubExport { .. } => "epub_export",
        Command::SnapshotPlan { .. } => "snapshot_plan",
        Command::SnapshotCreate { .. } => "snapshot_create",
        Command::PackageInspect { .. } => "package_inspect",
        Command::OpenFrozen { .. } => "open_frozen",
        Command::RestorePlan { .. } => "restore_plan",
        Command::RestoreApply { .. } => "restore_apply",
        Command::MigrationPlan { .. } => "migration_plan",
        Command::MigrationApply { .. } => "migration_apply",
        Command::V1ImportPlan { .. } => "v1_import_plan",
        Command::V1ExportPlan { .. } => "v1_export_plan",
        Command::ExposureGet { .. } => "exposure_get",
        Command::ExposureSet { .. } => "exposure_set",
    }
}

/// Whether this op is listed by MCP `tools/list`.
pub fn mcp_visible(op: &str) -> bool {
    COMMANDS.contains(&op) && !MCP_EXCLUDED.contains(&op)
}

/// Tool names in catalog order.
pub fn mcp_tool_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().copied().filter(|op| mcp_visible(op))
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, MCP_EXCLUDED, command_op, mcp_tool_names, mcp_visible};
    use omni_service::{Command, decode_request};

    #[test]
    fn catalog_matches_the_frozen_command_names() {
        assert_eq!(COMMANDS.len(), 47);
        assert_eq!(mcp_tool_names().count(), 47 - MCP_EXCLUDED.len());
        assert!(mcp_visible("library_list"));
        assert!(!mcp_visible("exposure_set"));
        assert!(!mcp_visible("library_open"));
        let request = decode_request(
            br#"{"wire_version":1,"request_id":"catalog","command":{"op":"library_list"}}"#,
        )
        .unwrap();
        assert!(matches!(request.command, Command::LibraryList { .. }));
        assert_eq!(command_op(&request.command), "library_list");
        for name in COMMANDS {
            assert!(mcp_visible(name) ^ MCP_EXCLUDED.contains(name), "{name}");
        }
    }
}
