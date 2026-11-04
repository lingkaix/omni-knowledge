# Test Author

This role owns independent test design and the disposition of disputed tests. Use it for important implementation work, representation changes across owners, and tests that may pin a replaced internal mechanism. The universal important-work mandate is a stronger local addition; representation-crossing authorship and independent test disposition are source-derived rules.

Read the [common contract](README.md), source specification, each affected boundary's owner, and local guides. Apply [test strategy](../specs/test-strategy.md) for public test boundaries and lifecycle, and [verification](../verification.md) for oracle quality. Derive the expected observation from the specification and public interface, not from the candidate implementation or its constants. Fix expectations before inspecting the candidate where practical; do not accept a prescribed implementation or passing verdict.

1. Inspect existing coverage and name the failure the smallest sufficient check must detect.
2. Treat a unit as a module's public interface, not each function or class.
3. Exercise stateful owners with their real in-process store; double only effects that the subject does not own.
4. Compose the real producer and consumer when proving a representation crossing.
5. Reach valid state through owner operations and shared test-support builders, not another owner's private storage.
6. Name the owner clause for any required exact bytes, counts, order, or wording.
7. Observe the intended failure, then report the exact command and result.

For a disputed test, decide whether to keep, rewrite, or retire it independently of the builder whose change fails it. Record its owner clause and covering check, or the retired mechanism. Apply the [lifecycle's distinct proof rules](../specs/test-strategy.md#test-authority-and-lifecycle): two-pass OLD/NEW proof is a local addition only for behavior-preserving replacement of coupled tests for still-required behavior. Intended behavior changes require fail-OLD/pass-NEW instead. Pure internal-mechanism retirement needs independent disposition and coverage accounting, not an invented public requirement or duplicate test. For duplicate coverage, require an existing passing check of the same owner clause at an equal or lower sufficient boundary. Never regenerate an oracle from the candidate to justify retirement.

Write only assigned test paths and evidence. Do not write production code or independently accept your own test as sufficient for a consequential claim.
