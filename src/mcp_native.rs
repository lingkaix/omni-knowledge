//! Frozen native-owner ports for mechanical CLI/MCP adapters.
//!
//! Transport data is never authority. Establish the actual native `Owner`/`Selection`,
//! call `connect_selected` and obtain `native_scope` for the actual routed command.
//! Reuse the existing queue's real Claim and complete prepaid requirement; preserve Waiting.
//! Query responses retain their owning encoded bytes through the last transport consumer.
//! User-visible output uses `NativePublicationPlan`, its matching planned operation and `publish`;
//! raw M3 staging methods confer no destination authority. Keep the result/error owner in the
//! foreground until real native collection and settlement finish. No static residue is success.
//! Snapshot reads use `NativeTrustedArchive` and keep the same Claim live until the last reader.
//! Startup/watch refresh uses `prepare_corpus_refresh` and `activate_corpus`, then settles its
//! existing job only after the final source/tuple consumer. No transport dispatcher lives here.

pub use crate::native_service::{
    EncodedResponse, NativeAdmitted, NativeAdmittedError, NativeCorpusDraft, NativeCorpusError,
    NativeCorpusRetirement, NativeDerivedRecipe, NativeEpubMember, NativeEpubPlan,
    NativeFrozenReadPlan, NativeLibraryService, NativeProcessing, NativePublicationPlan,
    NativeQueryController, NativeQueryInputs, NativeRefreshJob, NativeSnapshotError,
    NativeSnapshotPolicy, NativeSnapshotRequest, NativeSource, NativeTrustedArchive,
    NativeTupleQuery, NativeVideoPlan,
};
