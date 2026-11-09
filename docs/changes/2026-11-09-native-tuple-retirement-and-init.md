# Native tuple retirement and fresh initialization

`NativeCorpusDraft::retire_previous(self, progress)` returns the actual draft plus
`NativeCorpusRetirement { retired, pinned, retained_unknown }`. It requires that
same draft's durable complete selection, live Refresh Claim, qualified namespace,
and held exclusive WRITER. Reports contain observations and cannot authorize cleanup.

The existing prepaid metadata Work covers bounded manifest replay and hashing;
retirement does not allocate a corpus-wide member table. It exclusively locks each
previous generation's READERS before opening components, validates its library/root,
seal and complete tree, and preserves the whole tree when unknown material appears.
It rechecks selected CURRENT/WRITER, generation namespace, and prior controls before
controlled unlinks. The last actual source/component consumer retains its aggregate
pin even after the top reader is dropped. Retry retirement after that consumer ends.

Selection still marks Switched before fallible durability effects. The owning draft
already resolves a pending switch on drop; no change to that existing behavior was
needed. Tests kill exact actual writers before and after the switch, independently
reacquire WRITER, and open the selected tuple in a fresh native Query process. These
controls exercise process interruption, not power-loss simulation.

Fresh legacy CLI initialization now selects the private OS cache on the actual
publication device, falling back to its existing temporary cache when that device
matches. Explicit OMNI_CACHE_DIR remains authoritative; the process still has one
account, and the existing publication-volume check remains enforced. Cache ancestors
are qualified before creation, including aliases into the authored library. Init
continues to create its index and the resulting library remains searchable.
