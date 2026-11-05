# Knowledge Processing

The mature extraction, EPUB, media, preview, ASR, and runtime behavior lives here.
Root `extract`, `media`, `epub`, and `runtime` remain re-export shims.
[Processing](../../docs/specs/processing.md) owns formats and clocks; [storage](../../docs/specs/storage.md#effective-resource-accounting) owns resource accounting.
There is no second queue, resource pool, fabricated content digest, disabled processor, or metadata-only substitute for an admission failure.

## Fund the actual call

Production reuses `knowledge_core::admission::Account::current()` from Core P1 `d51cdde` / local prerequisite `e6b7f0c`.
The process owner starts that account once on an already admitted private cache directory.
`Funding::Claim { claim: &mut Claim, expected: Key }` consumes the existing claim with `Account::split_claim`; it does not reserve the same resources again.
`Key` comes from the trusted native owner; this crate never constructs a source digest or a `VerifiedSource`.
`Funding::BeforeVerification` is for actual pre-verification work and compatibility callers, not a way to hide job-caused costs.
No new Core API is required.

A call plans all concurrent input, returned data, capture/JSON buffers, child-group RSS, and generated storage before payload allocation, child startup, or generated writes.
The complete peak is checked against the whole process limit before checking current availability.
Impossible work is `TooLargeMemory` / `TooLargeStorage`, not perpetual waiting on separately possible components.
Contention is `WaitingMemory` / `WaitingStorage`; pre-effect component rollback leaves no partial charge or output.
Funded calls with insufficient remaining execution credit return `InsufficientCredit`, not a second reservation or fake waiting state.
A pre-effect record/transfer failure finishes already transferred components back to the account; it does not pretend to restore them to claim execution credit.
The native executor inspects remaining execution credit and settles the existing claim truthfully.

The caller supplies byte/file limits and child/storage promises within the existing Core work maxima; resource promises do not redefine `backend::Budget::max_memory_bytes`.
The runner polls the real claim cancellation state and deadline during I/O and RSS observation.
`None` for RSS uses only credit left after all planned concurrent buffers, not the whole account in addition to those buffers.
There is no fixed 512 MiB child ceiling or whole-corpus cap.

## Compiled funded descriptor API

`Funding` is available at `knowledge_processing::funding::Funding` and through each Root extraction/media/EPUB shim.
Existing signatures remain; the new functions are additive.
These signatures are exercised by actual integration tests; the example below is also compiled as a Rustdoc test.

| Function | Result |
| --- | --- |
| `extract::extract_file_funded(file, hint, max_bytes, child_rss, funding)` | `Admitted<Extraction>` |
| `extract::preview_file_funded(file, hint, max_bytes, line, child_rss, funding)` | `Admitted<String>` |
| `epub::read_epub_file_funded(file, max_bytes, child_rss, funding)` | `Admitted<String>` |
| `media::parse_transcript_file_funded(file, child_rss, funding)` | `Admitted<Vec<TranscriptSegment>>` |
| `media::segment_video_file_funded(file, dir, seconds, ClipPlan, funding)` | `Admitted<Vec<MediaSegment>>` |
| `media::transcribe_file_funded(file, whisper, model, dir, AsrPlan, funding)` | `Admitted<Vec<TranscriptSegment>>` |
| `epub::export_epub_funded(root, output, title, include_media, child_rss, storage_bytes, funding)` | chapter count after actual generation and no-clobber publication |

`child_rss` is `Option<u64>` and `funding` is `&mut Funding<'_>`.
`ClipPlan` contains `child_rss_bytes: Option<u64>`, `storage_bytes: u64`, and `max_segments: usize` (positive, at most the retained 100000 ceiling).
`AsrPlan` contains `child_rss_bytes: Option<u64>`, `storage_bytes: u64`, and `transcript_bytes: usize` (positive, at most the retained 16 MiB ceiling).
Generated-storage promises must be positive; one byte is a real one-byte extent limit, not permission to write first and refuse later.

```no_run
use std::{fs::File, path::Path};
use knowledge_core::jobs::{Claim, Key};
use knowledge_processing::{admission::Admitted, extract::{self, Extraction}, funding::Funding};

// The native owner supplies the already admitted FD, real claim, and exact retained work facts.
fn process<'a>(file: &File, hint: &Path, claim: &'a mut Claim, expected: Key<'a>)
    -> anyhow::Result<Admitted<Extraction>>
{
    let mut funding = Funding::Claim { claim, expected };
    extract::extract_file_funded(file, hint, 4096, Some(64 * 1024 * 1024), &mut funding)
}

// Only after the actual consumer/pin no longer uses the bytes:
fn retire(result: Admitted<Extraction>) -> anyhow::Result<()> {
    let Admitted { value, retained } = result;
    drop(value);
    for mut holding in retained { holding.finish()?; }
    Ok(())
}
```

## Held-FD native output seam

Use `epub::export_epub_held_funded(members, output_file, title, include_media, child_rss, storage_bytes, funding)` and `media::segment_video_held_funded(source, output_directory, seconds, ClipPlan, funding)` after native source/output selection.
The older `export_epub_funded` and `segment_video_file_funded` still accept paths; they are compatibility calls, not this native selection boundary.
`held::HeldSource { file, expected_version }` borrows an actual regular `File` and the native owner's expected `ContentDigest`.
`held::EpubMember { source, logical_name }` supplies a borrowed `knowledge_core::data::RelativePath` as publication data only.
The encoder resolves contained chapter/media links only among explicitly selected logical names; it does not scan or reopen a library root.
Approved `EpubMember` order is the EPUB spine and chapter-body order. Logical-name lookup resolves contained links and does not sort that sequence. The compatibility directory export still orders discovered pages by relative path.
Chapter/media limits remain 64 MiB / 128 MiB and the total encoded source/media limit remains 512 MiB.
The same mature encoder, XML serializer, sanitizer, ZIP writer and FFmpeg recipe serve both APIs.

The native owner creates the EPUB output as a new empty writable private regular inode with one link, on the selected generated volume.
Video accepts an already qualified private directory FD and exclusively creates flat `segment-NNNNNN.mp4` names with `openat`, `O_EXCL` and `O_NOFOLLOW`.
No call changes permissions, traverses an output ancestor, overwrites an existing artifact, installs a destination, or grants access from a name/digest/File alone.
Native policy, current context, expected identity, placement outside source/home/sync trees, owner intent, claim identity and final publication remain caller obligations.
Use the same actual verified `Key` and `Funding::Claim`; neither an approval boolean nor a claimed digest replaces those obligations.

Both calls prepay simultaneous manifest/record/pin buffers, transport, verification scratch, opaque child-group RSS and generated extents before allocation, serialization, execution or writes.
The parent samples native before/after stamps and runs actual descriptor hashing in the same watched parser before and after encoding.
Potentially blocking source reads never enter the parent pipe/watchdog loop; verification uses the same claim, RSS envelope and bounded worker collection.
The EPUB worker also verifies the exact captured member bytes passed to the encoder.
Video uses FFmpeg's seekable `fd` protocol and a child-only inherited directory CWD; it does not use a nonseekable stdin pipe, `/proc` URL, or original pathname.
The installed FFmpeg and ffprobe must support the `fd` protocol; absence is a real processing failure, not a fallback to a source path.
The parent never clears its own close-on-exec flags.
The caller must exclude concurrent source mutation and cursor use; collected media invocations restore the shared cursor.
Unknown collection does not restore a cursor that a live child might still use.

Results are `Admitted<held::HeldEpub>` and `Admitted<held::HeldVideo>`.
They retain actual output FDs, selected source FDs, the video directory FD and record/remaining-storage holdings through native consumption/publication.
Destroy the value before finishing the returned holdings; an account/claim settlement does not retire a live result.
There is no automatic unlink, cleanup by pathname, or implicit publication.

A settled failure after work starts is `held::PartialOutputs`, with the actual stopped files, source/directory leases, original cause and live holdings.
Destroy those records after native inspection/cleanup, then finish their holdings.
`held::SourceChanged` identifies failed native version checks; admission/stop errors before effects remain their existing typed errors.
Unknown child collection remains the existing `worker::UnsettledChild`, not a stopped partial or success.
Its descriptors retain sources first, then the optional video directory, then generated artifacts; artifact labels are relative data, not path grants.
Inspect `descriptors()` without reopening paths; after successful `collect`, transfer the actual leases with `take_descriptors()` and reconcile unknown output deltas before finishing the returned costs.
The existing `funding::UnsettledWrite` retains its primary output plus `leases()` and record holdings if extent reconciliation fails.
After `retry` succeeds, `take_retained()` transfers those record holdings; destroy the records before finishing them.
Retain all native cache/context leases outside this crate through either unresolved owner.

```no_run
use std::fs::File;
use knowledge_core::{data::{ContentDigest, RelativePath}, jobs::{Claim, Key}};
use knowledge_processing::{admission::Admitted, epub, media, funding::Funding,
    held::{EpubMember, HeldSource, HeldEpub, HeldVideo}};

// The caller has already performed current native policy and output-namespace qualification.
fn publication<'a>(source: &File, version: &ContentDigest, name: &RelativePath,
    private_output: &File, claim: &'a mut Claim, actual_key: Key<'a>)
    -> anyhow::Result<Admitted<HeldEpub>>
{
    let member = EpubMember {
        source: HeldSource { file: source, expected_version: version }, logical_name: name,
    };
    epub::export_epub_held_funded(&[member], private_output, "Selected publication", false,
        Some(128 * 1024 * 1024), 8 * 1024 * 1024,
        &mut Funding::Claim { claim, expected: actual_key })
}

fn clips<'a>(source: &File, version: &ContentDigest, private_directory: &File,
    claim: &'a mut Claim, actual_key: Key<'a>) -> anyhow::Result<Admitted<HeldVideo>>
{
    media::segment_video_held_funded(HeldSource { file: source, expected_version: version },
        private_directory, 10.0,
        media::ClipPlan { child_rss_bytes: Some(256 * 1024 * 1024),
            storage_bytes: 32 * 1024 * 1024, max_segments: 16 },
        &mut Funding::Claim { claim, expected: actual_key })
}
```

Linux owned tests observe actual EPUB output, tail-indexed seekable MP4 conversion, root/output-parent replacement, version refusal, prepaid extents and real-claim lifetimes.
The native callers in M6 and M5 must still bind this committed API into their own operations; processing does not claim that migration is complete.
macOS descriptor inheritance, private CWD, FFmpeg protocol support and actual new-API effects remain open native CI observations; no SDK installation or Mac runtime proof is implied.

## Native caller and buffer lifetimes

`hint` supplies only a bounded native filename/extension; bytes come from the admitted descriptor, not a reopen of that path.
The bounded `pread` read keeps the caller's cursor unchanged and cannot grow its allocation beyond its prepaid extent, even if the inode grows after metadata inspection.
An unlinked FD is not current-version proof: the owner must establish native containment, access, expected identity, and before/after byte-version validity before publishing a cache result.
The owner must retain borrowed source FDs, native cache-directory leases, and source pins until all children and I/O are actually collected, including after an unresolved error.
No API here creates source-admission authority.

Input and transport holdings outlive the actual input/capture buffers and die only after those buffers are destroyed.
Text copies and typed result decoding are prepaid, not charged after allocation.
Untrusted transcript `serde_json::Value`, opaque document/preview rendering, and EPUB encoding run in the actual RSS-watched parser process.
Returned holdings cover real vector/String capacities and typed records, not just text lengths.
Opaque/result envelopes conservatively retain their prepaid capacity until the result is freed; they are not a claim that every promised byte is resident.
Plain UTF-8 extraction retains the exact text-copy charge.
The owner must drop the value and then finish every returned holding; dropping a holding alone releases nothing.

## Generated writes and unresolved ownership

Generated regular-file destinations are bound to the account's selected native device before growth.
FFmpeg writes an exclusively created selected-volume descriptor; it cannot overwrite another output.
Whisper retains its required basename protocol inside the caller's admitted, exclusively owned cache directory.
The kernel `RLIMIT_FSIZE` is installed in the child before exec for the prepaid remaining output extent; there is no `RLIMIT_AS` or RSS rlimit.
Each conversion writes one known regular output; EPUB uses one parent-owned staging inode and no-clobber publication after actual collection.
Only newly observed extent deltas are reconciled with `written`, once per new inode, including known failed partial output.
Worker output and diagnostics are bounded nonblocking memory pipes, not unreserved temporary files.
Compatibility media storage estimates leave 1% native-observation headroom; funded callers supply their exact admitted extent without this estimate.

`worker::UnsettledChild` owns the actual child identity (PID/start), nonblocking pipe ends, child/transport/storage holdings, adapter-owned FD pins, and retained generated artifact paths when collection is unknown.
Collection is bounded to two seconds; kill/wait/observation failure never means completion or releases those costs.
The host retains the error, borrowed source pins and cache lease, inspects it, and calls `collect` only for that exact owner.
`collect` returns cost capabilities after real group/I/O settlement; the host reconciles actual artifact deltas before finishing storage credit.
Losing an unresolved owner intentionally strands its actual FD pins and Core credit until process exit, rather than allowing descriptor reuse to retarget a live child.
`funding::UnsettledWrite` separately retains the actual file, artifact paths and storage holding if a stopped writer's actual delta cannot be reconciled; `retry` never replays a successfully confirmed delta.
Neither error is `saved_local`, successful extraction, or claim settlement.
The low-level public `worker::settle(&mut Child)` now returns `Result<(), WorkerFailure>` for explicit bounded collection; `enforce_rss` reports a sample/refusal without implying collection.
All adapter callers were migrated to the shared runner; Root has no direct low-level helper calls.

Linux samples live owned-group `VmRSS`, uses non-reaping native exit observation to pin the leader identity, then collects non-zombie group members and pipe EOF before reaping.
The shared loop services bounded stdin/stdout/stderr batches and samples RSS/cancellation/deadline even while a tool is silent; there is no detached writer or blocking receiver/join.
Transient native exec/exit sampling gaps receive at most five further observations; a missing sample is never counted as zero memory.
Linux native controls cover real FFmpeg, killed/silent children, surviving descendants, escaped open I/O, and explicit recovery of retained costs.
macOS uses native libproc group/task observations and the same nonblocking runner for `textutil`; no native Mac qualification is claimed here.
Trusted tools must remain in their assigned owned group; escaped open writers are unresolved, not reported as collected.

## Root migration and compatibility limits

Root owners must switch actual job-caused processing to the funded descriptor APIs above and keep the returned holdings with the real consumer/pin.
Old `extract_file`, `read_epub_file`, `parse_transcript_file`, `segment_video_file`, and `transcribe_file` remain pre-verification compatibility wrappers, not funded-job entry points.
Path/preview functions still open once and keep their old result types; discarding their live holdings strands that charge until process exit.
This limitation is explicit, not a release or a complete Root wiring claim.
Compatibility wrappers retain their owned source FD in unresolved child errors.

At the reviewed `9a807f9` Root, migrate `src/lib.rs` extraction/parse-transcript/transcribe/segment calls (558/595/600/936), `src/main.rs` preview/extract/export (286/298/316), `src/mcp.rs` extraction (226), and GUI preview callers under their respective leases.
Do not convert `AdmitError`, `WorkerFailure`, `UnsettledChild`, or `UnsettledWrite` into metadata-only success; keep visible waiting, failed, or unresolved job state with existing Core semantics.
Root library/main/MCP/kernel were not edited in this lane; only the three re-export shims expose `Funding`.
The selected installed tools, parser-worker basename/lookup order, explicit ASR executable/model requirement, MP4 H.264 width 640 / 25 fps / no audio recipe, contiguous source ranges, recording-clock timestamps, safe previews, and no-clobber behavior remain intact.
