# Native aggregate Refresh cause and distinct source facts

A full-corpus Refresh must not submit nested same-mount source jobs, fabricate original-file
versions, or call a source's completion as whole-job success. `NativeRefreshJob` now owns the
actual admitted Refresh plus an opaque cause over the actual complete M1 inventory/root/epoch,
optional actual selected model descriptors, and immutable metadata-input descriptor bytes.
Those bytes are the actual aggregate job SUBJECT, retained and hashed in place. Their key
labels/digest are DATA, not original-file facts or permission. SDK/M1/model capabilities stay
retained separately; the cause never retains the Permit (avoiding callback ownership cycles).

The bounded native command/model-manifest envelopes quote four MiB before any new descriptor
copy/native inspection. The actual selected-root context remains stable across source stages.
The existing sole scheduler submits one real job; its resource/request budget is checked.
Every source requires an opaque row from the SAME actual inventory owner, then a complete
metadata scope split from that SAME real Claim before FD inspection/hash. Source verification
polls the actual claim/deadline on controlled refills; no copied scalar generation grants work.
Source/key/FD callbacks die before exclusive exact source-scope rejoin; an unexpected cleanup
refusal remains an actual typed owner (`NativeRefreshScopeError`), not a status/DTO/quarantine.

M3 representation facts now come from the SAME actual verified source carrier's UUID, digest
and effective code/hint processor, separately from the aggregate scheduling key. An actual
negative control exposed the previous assumption: first source UUID was replaced with the
literal metadata subject. Old-facts control FAILED; distinct actual-source facts PASS.
Likewise, an actual one-source completion returned success/settled the aggregate prematurely;
that control FAILED. Independent-source completion now refuses with its owning native error;
the actual enclosing controller must retain the job through whole-tuple settlement instead.

Deciding isolated fixture: actual selected library, actual M1 scan, different Markdown and
HTML/Japanese sources, one actual queued Claim under ALL-global-credit pressure, real M3 parser,
no per-source child jobs. Each full read phase and source scope returns its exact envelope;
global availability stays zero, ONE job remains unsettled until final controller settlement.
Another real same-root/same-byte inventory cannot substitute its owner. Status/missing actual
model/undersized metadata constructors refuse and return their actual incoming credit.
Foreign-owner and parent/global-credit ablations independently FAIL; sources restored.
First compile/API-guess REDs and first-page-only foreign-fixture RED are retained: the latter
was a bounded cursor fixture mistake, NOT a diagnosed M1 unchanged-source defect. The fixture
now follows actual next_offset/progress and checks issues instead of inventing source absence.

This is a concrete multi-source caller/cause prerequisite, NOT whole-application activation,
full-corpus persistence/query, whole lexical/vector/source/model tuple publication/recovery or
retirement, WeMM loaded-weight/quality evidence, M5 host/trust activation, or M6/all47 effects.
