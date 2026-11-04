# Processing, Representations, And Model Providers

Status: Accepted full-version contract, 2026-11-04; no runtime or quality result is implied.
This document owns bounded source-to-representation processing, timed media/transcripts, provider identity, and indexable units.
[Storage](storage.md) owns admitted bytes, residency, cache, and job lifecycle.
[Retrieval](search.md) owns indexing, embedding orchestration, fusion, and query coverage.
[Service](service.md) owns user-visible commands, budgets, jobs, and locator DTOs.

## Shared Responsibility

Extract the existing mature extraction, EPUB, media, and embedding behavior into `knowledge-processing` rather than rewriting every format pipeline.
The crate depends on neutral core contracts and has no GUI/macOS-framework, OpenKit Policy, account, or global-library-state dependency.
Known platform/process/provider variants use small traits/modules, not a generic plugin registry.
A host supplies admitted handles/paths, temporary-output ownership, provider capabilities, deadlines, and budgets.
Return serializable facts and bounded representations; large originals never cross a control boundary as unbounded JSON/base64.
Later OpenKit consumers can use the same processing implementation through their supervised Rust boundary while retaining their own admission/governance.
This delivery makes no OpenKit code change or claim that host-specific PDF/runtime paths have already been retired.

## Indexable Unit And Provenance

An indexable unit binds `source_id`, original SHA-256, representation SHA-256, processor identity/options, input dependencies, locator, modality, text or vector input, and coverage.
Processor identity includes code/version, platform tool/version, options, and relevant model/package/space/preprocessing facts.
A changed source, corrected transcript, embedding-relevant annotation, model, or recipe invalidates dependent units.
Do not reuse a representation solely because file size/mtime or vector dimensions match.
Direct UTF-8/Markdown text keeps original line/byte coordinates, including frontmatter, without inventing a sidecar requirement.
HTML/EPUB/PDF/Office extraction uses representation coordinates and real page/slide/sheet locators when the parser supplies them.
Extracted character offsets are never original binary byte offsets.
A missing coordinate is explicit/unlocated, not an invented page or zero timestamp.
Time evidence uses the original recording clock and retains the original/representation digest relationship.

For a PDF parser replacement, preserve page boundaries, Unicode, failure classification, provenance, and budgets through independent real-output fixtures.
A parser returning one text string does not prove page-coordinate parity.
Reuse an existing capable parser API or report representation-line-only coverage honestly; do not fabricate page offsets to pass a fixture.
An existing host parser can be retired only after independently authored still-required behavior checks pass both OLD and NEW.
A parser-dependent representation digest can change and require rebuilding; byte-for-byte parser parity is not assumed.

## Formats And Bounds

| Input | Required behavior |
| --- | --- |
| UTF-8/Markdown | Bounded direct text, original line coordinates, inert frontmatter/extensions. |
| HTML | Sanitized extraction/preview, safe links and offline renderer assets; no source scripts or remote resource execution. |
| PDF | Mature bounded text extraction with declared original page provenance where available; image-only pages remain uncovered without optional OCR. |
| DOCX/PPTX/XLSX and ODF | Bounded archive/document extraction with real slide/sheet facts when available; metadata survives parser failure. |
| DOC/RTF | macOS textutil behind its platform adapter; Linux reports unsupported or uses an explicitly packaged mature processor, never assumes textutil exists. |
| EPUB | Ordered chapter extraction/preview, safe contained resources, Markdown/HTML EPUB3 export with navigation and optional selected contained media. No promised fixed-layout fidelity or interactive content. |
| Images/video | Metadata plus selected bounded visual inputs, ffmpeg clips/keyframes and preserved time locators; no automatic whole-NAS processing. |
| Audio | Imported transcript or explicit local ASR; no direct WeMM audio encoding claim. |
| Unknown binary | Path/title/description/tag metadata and explicit unsupported content coverage; original bytes stay intact. |

Current direct extraction defaults cap input and derived text at 8 MiB per source.
These are visible resource defaults, not a maximum admitted original file size or a kernel inventory limit.
Media has separate bounded range/job admission; a large recording remains referenceable and seekable without reading it as an 8 MiB text file.
Default semantic text chunks contain at most 1,600 Unicode characters with a small overlap and preserve source/representation locator mapping.
Oversized/failed extraction reports partial/unsupported/failed coverage instead of claiming an empty body.
Metadata remains searchable when a processor is missing or fails.
No macros, imported scripts, source HTML programs, shell text, or package hooks execute.

All parser/ffmpeg/ASR work uses bounded jobs/processes, finite deadlines, cancellation, bounded output, and private temporary paths outside home/mount/sync roots.
Pass process arguments separately with no shell interpolation.
Successful output is installed only as a complete cache object; a stopped processor cannot publish half a generation.
Clean owned temporary files on success/failure/cancellation; report retained partial outputs after forced termination rather than deleting unrelated paths.
Budgets apply to decompression/output as well as original input to prevent parser/archive expansion exhaustion.

## Video And Keyframes

Use the selected installed ffprobe/ffmpeg executables; tool discovery is platform-aware and works after Finder launch.
The retained default clip recipe is contiguous ten-second segments with a shorter final segment.
The default visual clip is H.264 MP4, width 640, 25 frames per second, without audio; record these options in processor identity.
Keyframe/clip choices are bounded and versioned; fixed segmentation is sufficient for this delivery.
Scene-change segmentation and OCR remain optional measured extensions.
Every segment records start/end on the original clock; ffmpeg's reset output clock is not evidence time.
The existing WeMM helper's packaged frame sampling/preprocessing belongs to its declared provider identity, not an unversioned app assumption.
Inspection defaults to 30 seconds; each clip conversion defaults to 120 seconds, with finite explicit overrides under the job budget.
Never overwrite an existing temporary-output target; private temporary-directory ownership handles failed partial outputs.
Large media range reads remain bounded and current-source verified; preparing clips does not copy the original library into the cache.

## Transcripts And ASR

Preserve imported `<filename>.transcript.json` companions and authored correction lineage.
Support OpenAI Whisper `segments` with second-based `start`/`end` and whisper.cpp `transcription` with millisecond `offsets.from`/`offsets.to`.
Reject negative, nonfinite, or reversed ranges; ignore empty text; cap transcript JSON at 16 MiB by default.
A text-only transcript can carry zero-to-zero unknown timing but must not claim a real located speech match.
Convert to millisecond service locators without changing the original recording clock.

Optional local ASR uses an explicitly selected existing whisper.cpp executable and model, never an implicit download or provider start.
Normalize the first audio stream to mono 16 kHz PCM WAV, then run bounded local recognition.
The retained command shape is `whisper-cli -m MODEL -f INPUT -l auto -oj -of OUTPUT`, with separate arguments.
Recognition defaults to a finite thirty-minute ceiling; callers can choose a smaller job budget.
Generated ASR, normalized audio, and partial recognition files remain derived external cache/temporary data.
A user-corrected transcript becomes visible authored text with a digest and recording link; invalidate embeddings when it changes.
ASR text is an observation that can contain errors, not a replacement for the source recording.
A deterministic/mock recognizer proves failure/protocol handling, not actual recognition accuracy.
Real ffmpeg and real selected ASR runs are separate observations with exact tool/model/fixture identities.

## Embedding Provider

`EmbeddingProvider` exposes `identity`, `embed_text`, `embed_image`, and `embed_video` with capability discovery, finite deadlines, and bounded request/response sizes.
The full identity includes model, package fingerprint, embedding space, dimensions, preprocessing, and supported modalities.
Bind that complete identity into cache keys and generations; same-dimensional spaces are not interchangeable.
Reject missing/mismatched identity, wrong vector length, NaN/infinity, zero norm, and malformed response structure.
A response failure aborts dependent publication and preserves prior complete state; it is not an empty successful semantic corpus.

Use the existing WeMM loopback HTTP adapter on macOS while its MLX/Core ML runtime stays an external process.
Do not expose the provider port by default or embed its model/runtime in a shared crate.
The retained default endpoint is `http://127.0.0.1:18789` and the model is `wemm-embedding-2b-apple-2048` with 2048 dimensions.
The declared package fingerprint and embedding space are verified against health/models discovery and every applicable response, not inferred from the model label.
Current package identity is `2d481d864704eaabf57a57e77544a6439dd8a524752f74ad5b8543b7f7fe3ecb`, with space `wemm-2b-apple-q8-g64-2048-2d481d864704eaab`.
Other identities require an explicit compatible new generation, not silent reuse.
Use `GET /health` and `GET /v1/models` before content requests; validate response model/dimension/space and the admitted package context.

Text requests use one `input` string with `model`; image/video requests use one `image_url`/`video_url` data URI in the existing message format.
The retained adapter limits are 40 MiB media, 48 KiB text, and 2 MiB response JSON per request.
HTTP deadlines range from 1 through 600 seconds with a 300-second default, further bounded by the owning job/request deadline.
Accept loopback HTTP only, normalize localhost to literal loopback, ignore ambient proxies, disable redirects, and reject credential/query/fragment/path-prefix URLs.
An image-query source must be admitted by source reference or bounded explicit owner-supplied input; never accept an arbitrary model-supplied host path.
This provider encodes no direct audio; ASR/imported text supplies that route.

Ship a deterministic Linux provider for contract, identity, numeric, timeout, and failure tests.
It proves no semantic quality, WeMM recall, Mac acceleration, or model hardware compatibility.
An optional small CPU provider needs declared model license, dimensions, package/space, and separate real-model observations.
Lexical/headless use requires no model download.
Do not add a provider plugin registry or change the user's library descriptor merely to install an external tool.

## Preview And Acceptance Boundaries

The desktop uses service-returned admitted bytes/representations; preview is not another extraction/search authority or a filesystem mutation API.
Retain safe Markdown/HTML frontmatter, offline Mermaid/highlighting, EPUB chapters, PDF/native fallback, supported media, and explicit unsupported-format messages.
A per-opened-library preview origin prevents obsolete resource URLs from reading a different newly opened library.
Confined paths, CSP, no remote frames/resources/source scripts, and safe explicit anchor/library/browser link routes remain required.
Preview resource reads default to 32 MiB; larger media uses bounded range responses and unsupported full reads retain native-open fallback.
Numbered text preview defaults to at most 20,000 rows centered around a requested distant line with an explicit window notice.
Native WebKit/codec/Quick Look behavior needs actual macOS observations; extracted Office/EPUB preview does not promise editable layout fidelity.

Compose real extraction/media/transcript output with real retrieval-unit consumption to prove digest/locator/dependency preservation.
Inject changed transcript/model/options, malformed provider identity/numerics, parser exhaustion, missing tools, cancellation, and partial process output.
Keep metadata discoverable and previous generations intact while reporting each failure honestly.
Actual ffmpeg/ASR/tool-discovery tests must observe their real processes; stable provider doubles cover only external effects outside the processing subject.
Human Mac qualification retains Chinese IME, Finder launch, real OneDrive/NAS, WeMM GPU/ANE quality/latency, and native prompts under the product contract.
