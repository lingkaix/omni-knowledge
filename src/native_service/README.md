# Compiled native source / processing / delivery boundary

Public types: `omni_knowledge::native_service::{NativeLibraryService, NativeSource, NativeProcessing, NativeJobPlan}`. Runtime/authority types remain in `omni_service::{runtime,authority}`. This is an additive native SDK surface, NOT another wire command, scheduler, pool, permission DTO, or competing writer.

## Canonical M1 asset consumer

`reconcile_inventory(owner, actual_refresh_permit, native_text_root, context_holding, &Preflight)` consumes the existing M1 Store/LocalMount with the **actual selected directory FDs** and current owner policy/exclusions. Both context and scan scratch are prepaid before inspection; legacy homes require explicit durable library identity upgrade. Actual `StatFirst` remains metadata-only; actual `FullHash` requests bounded verification. Child-owned homes and native bookkeeping are outside the parent source inventory.

`inventory.page(actual_offset, budget, holding)` retains one bounded actual journal page. Follow `next_offset`/progress/issue/EOF, not an assumed first-page corpus. Consuming `page.into_sources()` yields opaque `NativeObservedSource` handles ONLY for that complete durable scan; the real page/context credit survives their last dependent source/delivery. Decoded observations/UUIDs do not construct them.

`source_for_observed(&actual_permit, observed, verification_budget)` checks actual Process/Refresh intention before body access and binds M1's independent UUID to the existing exact native FD. Current backend metadata must also match that SAME FD, not an equal-byte replacement. Process selects its exact UUID; Refresh extraction is allowed, but provider transmission requires actual `embeddings:true` (or Process/Embed). The SAME actual Refresh frame is supported without recursive guards/pin cycles. This consumer does **not** activate complete application startup/Refresh jobs or replace compatibility resident Root search/browse. See `docs/changes/2026-11-06-native-inventory-source-consumer.md`.

## Source and current context

1. Keep the actual native `Session` from `NativeLibraryService::connect`, with real transport observations. `native_scope(&session, &existing_command, approval)` uses the SAME 47-command routing/native approval/read-policy checks and real frame holding as dispatch. It starts no effect. Domain hosts may instead use their existing dispatcher `&Permit`.
2. `service.source(&permit, &RelativePath, verification: knowledge_core::backend::Budget) -> Result<NativeSource>` checks the selected read policy BEFORE opening the source. It traverses descriptor-relative/no-follow from that permit's real selected root, not `kb.resolve(path)` followed by reopen. Verification observes its explicit bytes/deadline bounds and checks the current native policy/namespace/source at every controlled hash refill.
3. `source.file()? -> &File` returns that EXACT FD. `source.hint()` is only an extension/MIME hint. NEVER reopen the hint. Recheck `source.check()` before/after controlled processing and before each controllable output boundary. The held FD AND its current descriptor-relative name must retain the observed version; namespace/marker/read-policy/revision changes refuse.
4. `source.library_id()`, `context()`, `version()` are observed native facts. Context hashes held root identity, actual current policy/revision and marker; version hashes the complete actual held file. They are data, not authority tokens. No constant `selected` context/library or request digest substitutes for them.

This first source constructor accepts regular files. It does NOT claim a whole-directory content fingerprint. Directory export owners can borrow the selected root/policy/revision through `Permit::with_selected_context`, enumerate/read descriptor-relative under current path checks and retain actual member/directory pins with `retain_check`. Never reenter the permit while its selected-context callback holds the owner lock. Source/descriptor APIs grant NO destination/raw-write authority; effects still require the actual qualified owner/publication/native intent APIs.

## Real queue and Claim

`source.processing(actual_recipe_digest, actual_caller_processor_digest) -> NativeProcessing` combines the caller processor identity with actual consuming Root/M3 source and lock bytes. Recipe/processor digests describe real work/code, never permissions. Key source identity derives from the real relative path, keeping equal-content files distinct.

- `processing.key() -> knowledge_core::jobs::Key<'_>` borrows its actual context, selected library, source identity/version, recipe and consuming processor identity. Ordinary-local `mount=None` is the constructor's real backend fact, not a caller mount claim.
- `processing.submit(NativeJobPlan { operation_id, job_id, budget, reservation, priority, max_attempts }) -> Result<jobs::Submitted>` ALWAYS calls the existing actual `Account::submit`, including when another job is pending/unsettled. It returns the real visible job ID/created flag, preserving native TooLarge vs contention. Never replace submission with a waiting String or fabricate a numeric/pending grant.
- Retain this carrier in the EXISTING job owner under `Submitted.job`; do not create another queue/registry/executor. The shared scheduler's `queue().claim(now)` can select another job. Dispatch it to its existing owner; NEVER discard a mismatched actual running Claim.
- `processing.funding(&mut actual_claim) -> Result<knowledge_processing::funding::Funding<'_>>` checks the live native source and M1's exact claim generation/key/stop request. M3 performs actual native `split_claim` admission at spend; the funding view itself grants no resources or effects. Do not separately reserve the same funded child/input/output costs.
- Borrow the FD with `processing.source().file()?`, use the hint ONLY for format detection and pass the real funding view to EEE's `*_file_funded` calls. `processing.check(&claim)` is a controlled native source/claim boundary. Keep native guards for every later controlled member read/effect/delivery.

M1 production Core/local/storage and retained storage source match compiled `a1144464a6970e608cbbc67f09ff2716d654ba40`, including the `3550ae6` prerequisite; no activation based on f5's unchecked ABI. M3 EEE remains the actual frozen implementation, not a reopened callee body.

## Owned funded extraction

`processing.extract(actual_claim, max_bytes, child_rss_bytes) -> Result<NativeExtraction, NativeExtractionError>` consumes the SAME native source carrier and actual scheduler claim.
It calls EEE's public funded descriptor API, checks current source/policy/claim before and after processing, and keeps the whole `Admitted<Extraction>` result paid through its last reader.
`None` for child RSS uses only this claim's credit after the complete input/output/capture peak, not unrelated account availability.
Choose the complete simultaneous reservation in `NativeJobPlan`; this method never enlarges policy or manufactures credit.

Use `extracted.value()?` for a checked borrow and `extracted.source()?` to prepay the complete response/copy/encoder peak before constructing it.
Keep the response holding through real output settlement.
`extracted.complete()?` first destroys all extraction fields, finishes their dead holdings, settles the actual claim, and returns the SAME `NativeSource` for delivery checks.
An abandoned result settles as failed, cancelled or superseded only after actual resource destruction and native collection.
An unresolved error owns the actual source/claim/child capabilities; `collect()` makes one bounded native attempt and drop remains in foreground shutdown while collection is unknown.
For a mismatched dispatch, `error.into_parts()` returns the actual source carrier and actual Claim after real resource collection, without settling that Claim under another key.
No job registry, worker pool, or second account is introduced.

The compiled `native_extraction_owns_complete_claim_and_paid_result_lifetime` test uses the real opaque HTML parser and ONE actual Core claim.
This additive owner does not replace the lower-level source/funding/delivery API above.
The held staging calls below use the same owner; CLI/MCP/daemon publication and transport ports still belong to their existing owners.
Legacy raw compatibility returns are not native job owners.
See the [repair record](../../docs/changes/2026-11-05-foreground-native-extraction.md) for limits and gates.

## Private M4 text-index input boundary (not full index activation)

`NativeProcessing::extract_indexed(actual_claim, max_bytes, child_rss)` calls the SAME M3 funded descriptor producer. It splits a64KiB metadata sibling from that SAME complete live Claim BEFORE native work, leaving it outside M3's `None` child-RSS remainder. The submitted whole `NativeJobPlan` must include this sibling and all separately split consumer/publication work. There is no late global reserve, second account, processor, queue or model.

The resulting `NativeIndexInput` retains the actual M3 value/source/Claim and resource owner. Checked `text`, `source_id`, `source_digest`, `representation` and `source` borrows describe the actual original FD/key, exact produced text SHA and effective consuming processor fingerprint. Its modality is the text EMBEDDING INPUT; original binary MIME/type remains source-owned. Identity fields die before the owner releases metadata credit. This native key is not yet the complete M1 asset-ID/rename integration. Call `complete` only after actual consumers and their split buffers/artifacts are destroyed/settled.

The index-input consumer checks the exact native authority's retained `Process`/Extract command, selected source IDs/library and actual request budget before processing, not a fabricated status/browse grant. `Permit::admitted_command` borrows the sealed command after currentness checks; cloned permits share its Arc AND actual frame resources. Native manually constructed requests pass the existing bounded codec BEFORE that clone. This adds no command or wire authority; the old low-level caller/held M6 bridge still must supply its actual operation and separate destination intent.

Builder controls use real HTML/Unicode, the actual Process scope (read-only refuses; an explicitly controlled native owner chooses the positive read/write policy), actual queue/Claim and M3 parser, then unpublished vector members through exact retained FDs. Precomputed2D numbers test IO/binding only, NOT an embedding backend or semantic quality. The pre-seal status-permit control actuallyFAILS; restored scope binding passes. Removing only native command-size validation actuallyFAILS its oversized-request control. Root30/service119/wholeworkspace strict pass; intermediate import/read-only/private-helper REDs retained. The new global-queue fixture runs in its own account process, with child stdout captured so existing inventory counts are unchanged.

Actual model/provider execution, multi-source whole-tuple production/publication/recovery/retirement, full-corpus Root orchestration, transport/destination authority and native/full-product acceptance remain open.

## Private native embedding owner composition (not full M4 activation)

`NativeLibraryService::select_embedding` requires the actual separate SDK `Owner`, actual read-only package and complete model-JSON descriptors, explicit receiver configuration, bounded verification budget and an ALREADY prepaid `Work`. It streams the package through one paid64KiB window, matches its actual SHA with the full nine-field model identity, retains BOTH exact FDs/native stamps and guards the actual selected library/configuration epoch. No hash-and-path reopen, source/receiver grant from loopback, descriptor digest, or DTO is used. Owner-only selection commits the SAME SDK selection epoch only AFTER all package/metadata preparation succeeds, without resetting current policy/exclusions. A stale/equal-label selection from another authority cannot substitute. This validates chosen artifact/receiver context, NOT by itself what weights an external process loaded or its model quality.

`NativeProcessing::extract_for_embedding` is ONE consuming factory over the SAME actual Claim. It checks the sealed actual Process/Embed/source/budget, the selected model's opaque permit identity, and admits the full funded provider BEFORE the existing M3 descriptor producer/`None` RSS remainder. The opaque output state owns provider/runtime/credit, selected model pins, actual extraction/facts and original source/Claim throughout requests/results. No separate Core-generation API, source-FD clone, model registry, processor engine or resource pool was added. Native model and consuming SDK/code identities enter the effective processor fingerprint.

`NativeEmbeddingProducer::embed(range)` consumes the producer for an actual extracted UTF-8 byte range; `NativeEmbeddedText` exposes only checked borrows/full dependency/model facts. Shared vector dimension/finite/nonzero/declared normalization validation runs before output exposure. `into_producer` destroys the vector first; consumer copies need their OWN prepaid buffers. Complete only after actual consumer/publication/delivery settlement. Errors retain the actual generic native owner and recover the exact original Claim; neither completion nor numeric output is an index-publication/quality receipt.

Native `CurrentContext` now retains its own prepaid64KiB marker FD/scratch lifetime, with checks serialized by that SAME selection lock. It no longer makes late global reservations during a fully funded source/model/HTTP request. The actual all-free-credit pressure control fails OLD with false source revocation and passes NEW. This does not imply unlimited startup/config parsing or physical RSS acceptance.

The owned model-code/metadata/receiver fixture composes actual HTML/Japanese M3 output, actual Process/Embed, selected package FD SHA, exact Claim, HTTP numeric generation and prepared exact-FD vector rows with ALL otherwise-free global credit held. It checks wrong Extract intention/foreign equal-label authority BEFORE contact, invalid package preserving old configuration, equal-byte model metadata replacement, non-resurrection, and invalid declared L2 before exposure. Separate source-binding, model-FD, package-hash and Embed-intention ablations actuallyFAIL; restored affected Root/processing/search/SDK349runtime+5guides and workspace strictPASS. It is an explicit deterministic CONTRACT model, not WeMM/semantic quality, Mac acceleration or human-native acceptance. Native M1 asset-ID/rename, full multimodal providers, bounded full-corpus generation/orchestration/whole-tuple publication/recovery/retirement and M6 transport/destination/effect activation remain open.

## Private native Query continuation (not complete application Query activation)

`NativeTupleQuery::embed_keyword` always returns the actual selected Query/model/Claim owner, including failures. Explicit `materialize` prepays a new vector buffer from that SAME Claim before copying, then destroys the original output/provider/runtime/IO before rejoining its actual loans. The small live copy retains its own actual Holding. `finish` destroys data before exact rejoin and returns the actual owner if cleanup cannot complete. Known keyword/vector DATA is not complete HTTP transport-data or loaded-weight qualification.

`vector_requirement` derives DATA from the actual held source table using the existing mature vector reader formula. A bounded `NativeQueryStage::semantic` uses that actual Claim's reader Work, selected tuple generation/full model identity and actual M1/original source SDK owners. It delegates cosine/distinct-file ranking/evidence to the SAME vector engine, admitting the exact tuple-bound metadata record span and checking source/digest/processor/representation/global-span/current native source before distance. Equal-byte windows still need their own admitted global span. `NativeSemanticRun` retains real result/error/reader/stage/source owners, rechecks access and destroys data/FD consumers before exact reader-credit rejoin. Empty eligible sources open no vector components. This is one bounded source set, not whole-library semantic coverage.

`NativeQueryPartition::scan` uses the SAME actual SourceGeneration for selected literal/regex methods and original coordinates. This primitive needs the whole-library controller below for every admitted representation, canonical metadata, fusion, labelled libraries, and final delivery; application activation remains separate. See the [scoped ownership and ablation record](../../docs/changes/2026-11-07-native-query-provider-vector-and-scan-owners.md). Numeric HTTP/vector controls do not qualify loaded WeMM, quality, native Mac, installed product or physical M0.

## Whole-library Query and encoded delivery

`NativeTupleQuery::start_controller(actual_claim)` composes the requested methods over every held library without entering the host mutex.
The product caller must first use `NativeQueryInputs` to acquire all selected-library input owners under the same SDK invocation and original deadline.
`requirement_all` quotes the maximum sequential method peak plus the simultaneous retained lists for every library, method, and keyword; a quote grants no resources.
`NativeWholeSemanticRun` makes one vector-row pass with current source eligibility, while `NativeScanRun` visits every admitted lexical representation.
The controller fuses distinct file ranks per method/keyword and keeps independent libraries in labelled groups.
Each library retains its own winning source and pointer scopes through final result checks.
Coverage counts come from the requested streams before result limiting; a metadata method reports current M1 observations, and an indexed method reports the admitted indexed stream.
Processing statuses observed in an indexed stream remain in combined coverage.
Representation positions are mapped to real lines through bounded prepaid tape reads; only identical original and full representation digests permit original byte coordinates.

`search_result()` rechecks the actual Claim and winning dependencies before borrowing the result.
`encode_response(request_id)` performs the same checks, prepays copied DTO and encoder capacity, and returns `EncodedResponse`.
That carrier owns its bytes and existing `Work`, implements `Deref<Target = [u8]>`, and can move into an asynchronous transport or an `Arc` without tying its lifetime to the controller.
The transport keeps the carrier until its final write consumer ends; bytes are destroyed before the credit is released.
Controller `finish` closes its retained sources/pages before rejoining their loans and returns its actual owner if a scope consumer remains.
An encoded carrier can outlive controller and Claim settlement while retaining its independent charge.
See the [controller change record](../../docs/changes/2026-11-08-native-query-controller-delivery.md).
Application face routing, tuple activation/recovery/retirement, trust/intent/drain activation, loaded-model quality, and installed/native platform acceptance remain separate work.

## Held EPUB and video staging

Committed M3 ABI `d6ae37d16ddbdde3b33086293f2db2a85eaaef1c` is consumed without editing its producer bodies.
`NativeAdmitted<T>` and `NativeAdmittedError<T>` generalize the same paid owner; `NativeExtraction` and `NativeExtractionError` remain aliases for the original extraction contract.

- `processing.export_epub(actual_claim, Vec<NativeEpubMember { source, name }>, NativeEpubPlan { title, include_media, child_rss_bytes, storage_bytes })` calls `epub::export_epub_held_funded` with actual admitted FDs and versions in the supplied spine order.
- Each member is an independently current `NativeSource` from the same selected context.
The actual queued recipe must bind ordered logical names, source versions, title and media options; names are link data, never filesystem lookup authority.
- `processing.segment_video(actual_claim, NativeVideoPlan { seconds, clips: media::ClipPlan })` calls `media::segment_video_held_funded` with the SAME held source/version and an actual private directory FD.
- Root splits namespace/metadata costs from that actual Claim before creating staging files.
The private namespace, original FDs, all member source owners, typed M3 errors and complete returned holdings stay with the result/error.
No path reopen is used to consume generated EPUB or clip bytes; borrow the actual returned File.
- New native member pins are prepaid before duplication and retained in the final anchor's actual delivery scope.
Members from separate scopes therefore remain current-version checked after processing completion.
- `value()`, `source()` and `complete()` retain their extraction semantics, including all-member current checks and last-reader destruction before credit release.
`complete()` destroys staging data, so an authorized publication/copy owner must finish its real consumer first and prepay any new buffers before copying.
No staging method grants raw writes, destination authority, publication, or `saved_local`.

Typed `PartialOutputs`, `UnsettledChild` and `UnsettledWrite` stay owned, never stringified into settlement.
Actual stopped partial records are destroyed before their credit finishes; unknown writes are not reconciled by guessing artifact sizes.
A changed staging namespace refuses retirement while its real owner and credit remain reachable.
`error.staging_directory()` exposes the actual retained FD for native recovery, not disclosure or destination authority.
Controlled hosts must retain these carriers in their existing job owner, collect/drop them before termination, and never call `process::exit` while unknown work is owned.
The SDK does not install a second signal runtime or certify another owner's shutdown hook.

`actual_root_held_epub_and_video_keep_funded_native_owners` consumes real generated EPUB/MP4 bytes through FDs, checks reversed-name spine order and Unicode, separate-scope member drift, actual Claim/reader credit lifetimes and unchanged sources.
Native Mac effects, full face/publication wiring, legacy vector-provider pathname consumers and frozen B2 allocation remain OPEN.

## Settlement and Delivery

Destroy ALL paid payload buffers (including format/status/warnings, metadata and entry collections), FDs and temporary objects before their particular `finish`/`rejoin`. Return only exact synchronous loans AFTER their real buffers die. Root's funded resident-copy envelope is prepaid before body construction; no live/cached parent holding is rejoined merely to resize it. Native source/key/descriptor costs remain separately owned through their actual last use.

Only after actual child/IO settlement use the EXISTING queue settlement API with `processing.key()`. `into_source()` then retains the same current source/delivery owner. Unknown child/write settlement must retain the COMPLETE `NativeProcessing`, actual Claim, M3 unsettled capability, descriptors and costs and continue bounded collection or a real existing-owner transfer. A receipt/string/global about to disappear at `process::exit` is not that transfer.

`source.reserve_result(bytes)` admits the full simultaneous response/escaping/clone/encoder envelope BEFORE growth, not a guessed `3 * text`. For ordinary native CLI output keep the source/permit owner with all paid bytes through the actual bounded writer/flush result; use `source.check()` before each controllable write. Do not wrap an arbitrary CLI payload in a fake wire result.

For an EXISTING v1 `Response`, `source.into_delivery(response)` moves the same guard/holding into `runtime::Delivery`; native domain owners may use `Delivery::from_native(response, permit)`. Both require pre-admitted construction/encoding. Hold `Delivery` AND encoded bytes through real write settlement, check before every controllable chunk, and never write old encoded bytes following a refused check. `encode()` returning is NOT transport settlement. Writer/deadline/error handling remains the transport owner's actual implementation, not this API's certification.

## Executed boundary proof / scope

`native_source_real_claim_key_and_delivery_bridge` executes real descriptor hash/version binding, two visible jobs while one is running, actual EEE funded-FD extraction, whole-value destruction before exact rejoin, terminal/stopped claim refusal, native queue settlement and source-change refusal after encoding. Original seven native controls remain active.

This compiled boundary does not certify the consumer's five caller repairs, close consequential rereview of the three 9ed findings, or activate native M6. Generic held-archive lexical/vector IO and native provider ownership have separate qualified controls; full Root disk-vector/query activation, full-corpus streaming/complete generation selection and full native macOS acceptance remain required. No waiting for FSEvents and no host inotify changes/suppression are part of this slice.
