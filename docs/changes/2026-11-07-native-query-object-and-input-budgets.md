# Native Query object work, owning page refusals and input-page ceilings

## Scope

Extends the private [bounded BM25/data-read primitives](2026-11-06-native-bounded-bm25-query-data-budgets.md). This is not the complete all-method controller, cross-library input assembler, application activation, final delivery or product acceptance. Existing engines, original request ceilings, processing deadlines and source datasets remain; no new resource account, backend, index, score or replay engine is introduced.

## Object examinations are work, not returned-file counts

The existing per-call `Budget::max_files` means objects examined, including unavailable entries. Native M1 already counts its observation/checkpoint/event-cursor objects; cached source/representation examinations also consume work. These counts are separate from distinct returned files and from the catalog partition count. Re-examining a source is not free work or a reason to keep a corpus-sized seen set.

The same data allowance now restricts both bytes and object examinations. Separately funded actual input metadata and planning examinations are included in the sealed request, with a data-only baseline at actual submission so an execution Claim does not retroactively pay inputs again. An opaque synchronous M1 call holds its whole possible byte/object span against this same allowance, then returns only the known unconsumed data span after the existing bounded decoder completes. Failed spans stay conservative. Core reservations, actual FD owners and original authority checks remain independent.

An actual Claim limited to 70 object examinations previously completed all nine native partitions over the existing 75-source corpus: the added control actually failed. The repaired path gives a fixed typed budget refusal, does not fabricate source absence, retains its actual run owner and performs no retry effects.

## Return the actual failed page loan

The first object-budget repair exposed an independent ownership defect: a page factory refused after its incoming Claim loan had already been split, and its old synchronous cleanup finished that credit globally. The actual exact-return control reported **209073680 instead of 211301904 bytes**, a lost **2228224-byte** page loan.

The existing inventory decoder is reused through `page_owning`; an error returns the **actual incoming Holding** after local replay data dies. The aggregate factory retains either that Holding or the **actual produced page/data/pins** in `NativeQueryPageRefusal`/`NativeQueryPageLoan`. `into_parts` returns the real owner; a DTO/error code cannot replace it. Controller cleanup first destroys page/source consumers, then rejoins the actual loans to the same Claim. It returns the actual run on a live consumer/refusal, never reserves replacement global credit.

Typed budget context preserves the owning error below it. The preexisting foreign-library negative caller now explicitly consumes its returned input loan: the test additionally proves its actual 1024 bytes remain reserved while the error is held, and retains the original exact post-cleanup reservation assertion. Dropping a bare Holding still releases nothing. This does not infer unknown group/thread/IO settlement from drop/EOF.

## Admission before native decoding; remove duplicate inspections

A stronger actual native-frame control caught late object admission that bookkeeping alone missed: **72 real source-table FD decodes + 34 actual M1 metadata objects = 106 examinations for 70 admitted**.

Admission now precedes each controller source-frame decode. The controller passes the SAME opaque `TuplePart`, actual source-table FD/version and original actual reader owner into source setup rather than decoding it again. Its sealed framed-byte length is quote data, not authority. Actual reader identity, selected policy, current root/member/rows versions, SDK invocation, source UUID/path, original FD/full stat, model, actual Claim, cancellation and deadlines are still checked before retained source setup and through consumers. No hash-FD-then-reopen, path/digest grant, permission cache or new lookup index is used.

The restored actual control observes **36 source FD decodes + 34 M1 objects = 70**, passes exact loan return, and leaves ordinary all-nine and the unchanged 22 MiB phase control passing. Byte savings are observed data-work reduction, not claimed RSS or retrieval-quality improvement.

## Input preparation also admits remaining work before each replay

A separate actual read-only M1 preparation control exposed **85 decoded metadata objects under a 70-object input ceiling**. The old code detected the cumulative limit only after another unchanged per-page preflight had already run.

Each subsequent call to the SAME existing decoder now receives only the remaining object, native-byte and original elapsed-time allowance. No further replay begins with a zero remaining dimension. Native budget refusal is typed `knowledge_storage::Error::Budget`, not a recovery/source-absence story. Post-read recovery/continuity/current-policy checks remain. The restored control decodes **70**, refuses before more work, and preserves ordinary, execution-object and 22 MiB controls. No caller limit or deadline was raised.

This fixes the actual one-library input replay loop. The final front controller must still share input limits/deadlines before opening all selected library inputs; summing already-read independent inputs after the fact is not aggregate pre-admission.

## Acceptance boundary

Actual isolated input/object/ordinary/phase controls, three local allowance tests and workspace strict have passed on the later source. The exact final full-source gate remains required before commit/publication. All fourteen corpus modes preserve prior cases; compile/zero-test attempts are nondeciding. Raw failures and ownership receipts remain outside the source tree.

Still unfinished: canonical metadata/tag/language eligibility, actual semantic model/receiver/vector bridge, mature candidate expansion and all-method/keyword fusion, exact/regex complete occurrence/evidence coverage, labelled library results, final actual delivery ownership, publication/recovery/retirement, remaining M2/M5/M6 authority/trust/draining repairs and native macOS/installed/loaded-weight/physical M0/full-product acceptance.
