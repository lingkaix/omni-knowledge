# Preparatory Bounded Cooperative Jobs

Status: additive preparatory mechanism from `1851eae69ebd9b252188502575165c7db74a2fb3`; independent public-API authorship/review and actual caller integration remain separate gates.
The engineer authorized one small queue/reservation owner, not execution, durable authority or a resource-policy amendment.
The accepted [storage owner](../specs/storage.md#bounded-jobs-and-cache) owns cooperative scheduling, reservations, cancellation, idempotency and supersession.
The [processing owner](../specs/processing.md#formats-and-bounds) owns actual bounded processors/output; the [service owner](../specs/service.md#jobs-mutations-history-and-package-dtos) owns the separate public Job/Budget wire mapping.
The [Core guide](../../crates/knowledge-core/README.md#preparatory-cooperative-jobs) specifies the actual mechanism's inputs, transitions and limits.

## Chosen Owned Behavior

One mutex protects canonical jobs, retained submission receipts, logical reservations and a checked instance-lifetime sequence.
Caller-selected capacities bound queued work, every retained receipt/alias, copied UTF-8 descriptor bytes, simultaneous working-memory/temporary promises and per-mount active slots.
Borrowed input is charged before copies; fixed record/vector overhead is count-bounded, with no capacity-sized preallocation or stale heap/history entries.
A duplicate key shares actual work, but a new operation/job alias needs a retained bounded receipt to prevent later different-intent reuse.
Different intent under a retained operation/job/key returns conflict instead of changing the admitted budget/priority silently.
Only explicit terminal retirement forgets the job and all its receipt aliases; it neither resets lifetime numbers nor retires durable data or external resources.

Priority/FIFO applies among eligible queued jobs; unavailable resource/mount slots do not prevent other eligible work.
Claim takes all logical promises and the per-mount slot atomically or takes none.
Unsettled tokens bind the same owner/job/claim generation and retain that owner without copying descriptors.
Cancellation, explicit same-subject supersession and cooperative deadline observation never release unsettled promises.
Actual trusted settlement releases once; losing a token strands capacity rather than inventing worker stop.
A claim still exposes its same owner after the original Queue handle drops; stale/duplicate/wrong-owner/context reports refuse without release.
A completed worker report may legitimately win a cancellation race.

Stopped offline work can wait for explicit reconnect within caller-selected finite attempts and pending capacity.
There is no automatic retry/backoff loop, worker pool, dependency graph, daemon, provider or process implementation.
Invalid models and other failures stay terminal until explicit host intervention/new work.
Monotonic deadline data does not install a timer or resource limit.
Supersession checks context/source identity and different key, never digest ordering, and does not itself submit replacement work.

## Necessity And Reasoned Alternatives

| Element | Present need | Effect of removal or substitution |
| --- | --- | --- |
| One owned state/lock | Atomic complete reservations under concurrent cooperative claims. | Separate checks/counters can partially admit competing work; three controllers duplicate ownership. |
| Finite jobs/receipts/text settings | Bound queued work and retained idempotency metadata. | Only bounding pending jobs leaves finished records or duplicate aliases unbounded. |
| Retained alias receipts | Bind every accepted operation/job ID to the shared work. | Key-only dedup can forget an accepted operation ID and later accept it for different intent. |
| Eligible priority plus original FIFO | Deterministic ordering while using available mount/resource slots. | A stale-entry heap grows on repeated transitions; strict head blocking strands unrelated available slots. |
| Explicit Unsettled state and owner-retaining claim | Keep logical resources charged until actual settlement, including token/initial-handle loss. | RAII force-release or cancellation-as-completion can start new work while the old worker is live. |
| Instance/record/claim binding and lifetime sequence | Reject stale finish after reconnect/retirement or identity reuse. | Reused indexes/IDs can release another job's reservation; wrapping counters can alias old claims. |
| Explicit waiting/reconnect/attempt bounds | Represent offline failure without hot retry loops or capacity spinning. | Automatic retry guesses connectivity/policy; treating offline as successful empty work fabricates completion. |
| Explicit retirement | Release finite metadata without secretly ending idempotency. | Automatic eviction silently permits operation replay; unlimited retention eventually has no bound. |
| Existing Core Budget/digest/fingerprint facts | Preserve domain ownership and avoid another DTO/identity implementation. | A copied wire schema or new recipe registry adds competing authority. |

No uncertain performance/removal claim needs an experiment in this additive slice.
The assigned external evidence log records exact builder operations, artifact/dependency identities, actual failures and cleanup.
Permanent regressions are independently authored after this public API freezes, not producer-tuned tests.

## Authority And Remaining Limits

All capacities, reservation costs, attempt bounds and admitted key facts are explicit trusted caller inputs, with no invented numeric defaults or physical-resource claims.
The original Core Budget remains work maxima; reservations do not silently redefine memory/input/output/process semantics.
The processor fingerprint must already include actual effective models/options/packages/dependencies, and source admission must already establish current qualified version/root/policy/residency facts.
IDs, hashes and claim bookkeeping never grant source/cache/backend/service access.
The effective accounting/default-policy and sequencing questions remain proposals for engineer acceptance; this record does not settle them.

No Root/service caller, existing API/cache/native/backend/manifest/data/wire implementation, dependency, Cargo/lock, test, owner or gate changes.
No actual RAM/disk/process enforcement, pin transfer, output publication, retirement, reaping, journal, restart recovery or full M1/M0 acceptance follows.
The runtime owner must retain/drain accounting while workers can remain live; a new empty owner or lost owner state never proves old workers stopped.
Lost unsettled claims have no forced-release recovery in this slice; durable operation storage, truthful cache lifecycle and separately qualified executor settlement remain prerequisites for integration that needs those guarantees.
