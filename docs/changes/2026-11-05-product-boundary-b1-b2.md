# Product Boundary B1 And B2 Observations

This record owns the new `tests/product_boundaries` oracle for two root-product resource gaps.
It does not change production code, manifests, locks, vendor sources, kernel tests, or existing suites.
It does not close B3, B4, PDF or HTML allocation, the Mac product-bound condition, or M0.
The kernel 48 MiB allocation oracle and the 512 MiB resident-set oracle stay unchanged.
Those kernel checks are not the ceiling used here.

## Public Subjects

B1 drives the root `omni serve-mcp` binary on stdio.
The public command serves newline-delimited JSON-RPC.
The existing frame ceiling is 4 MiB of line content.
A frame longer than that ceiling must be refused while stdin stays open, without a newline or EOF from the observer.
The observer must not treat a parsed initialize prefix or an echoed canary as success.
Valid LF and CRLF initialize requests, a terminated frame of exactly 4 MiB, a terminated frame of 4 MiB plus 64 bytes, empty EOF, a short frame ended by EOF, and a newline-terminated malformed UTF-8 frame keep their public outcomes.
The oracle does not match incidental error prose.
A protocol failure may close the process.
The test does not require the server to stay up after refusal.

B2 calls public `extract` with a caller limit of 1 MiB, below the documented 8 MiB default.
The fixture is a stored ZIP of a minimal ECMA-376 workbook.
One shared string is referenced 256 times.
The referenced text is 16 MiB, and three simultaneous copies stay under the 96 MiB host stop.
The input file must be no larger than the caller limit before the call.
Returned text must stay within that limit and must contain the computed cell text.
The owner caps derived text and says budgets apply to output as well as input.
The owner does not publish a peak-allocation or resident-set number for XLSX assembly.
The observation ceiling is four times the caller limit plus the admitted input length.
That ceiling is only a proof threshold for this fixture.
It is not a new product cap and it is not measured RSS.
The deciding failure is live requested-byte growth during the call, not the length of the returned string.

## Instruments

The process observer is admitted on stand-ins before any product case.
The stand-ins cover natural refusal, a lying success prefix with a canary, early stdin EOF, a hang, stdout closed while the child stays live, a terminated drain with no response, signal death, and a persistent success response.
A separate ownership check spawns a real child and settles that armed child immediately.
It does not inject a pipe or fcntl failure.
A second armed child is unwound.
Both paths must collect the owned pid. Cleanup does not use a blocking wait.
The allocator counter delegates to the system allocator and is admitted with an 8 MiB hold and a 4 KiB hold.
Those controller cases are not MCP or extractor results.
The stand-in is the same `product_boundaries` test with `OMNI_PRODUCT_BOUND_STANDIN` set.
That child performs only the named observer role.
With the variable unset, the test runs admission and the product oracle.
This target has no ignored test and no empty test.
The earlier ignored helper remains only on the preserved receipt commit.

A natural coded exit, a signal, a public JSON-RPC response or error, stdout EOF while the child is still live, and a timeout are different observations.
Stdout EOF waits for a natural exit until the same bounded deadline. It is not refusal by itself.
Signal death is not ordinary refusal.
The observer may kill only after that observation, to contain a live process or to reap a persistent server that already answered.
The kill is not the refusal evidence.
The primary outcome and a cleanup failure stay visible together.
The reap guard is armed at spawn, before pipe or `fcntl` setup can fail.
Settlement is marked only after `try_wait` confirms collection.

The exact-ceiling frame is one valid initialize request whose JSON text is 4 MiB, then LF.
Its public outcome is a JSON-RPC result or error for that request.
The oversized terminated frame remains 4 MiB plus 64 `B` bytes and LF, with stdin left open.
Its outcome is a natural coded exit or a JSON-RPC error, not a completed write.
LF, CRLF, and final-EOF controls still require their natural response or exit.
The overshoot, open stdin, canary, and prefix checks are unchanged.

## Case Count

The controller count is 13: CRC-32 vector, ten observer cases, the exact-ceiling fixture check, and one allocator case that runs both windows.
The ownership case settles one armed child immediately and unwinds a second armed child.
The unresolved-fixture case withholds collector status on a hang child, keeps that fixture, skips later product observations, then collects the same child.
The product count is 10: two XLSX cases and eight MCP cases.
One of the XLSX cases and the unterminated MCP case are the deciding regressions.
The other eight product cases preserve ordinary behavior.
The XLSX ceiling and fixture are unchanged.

## Proof Boundary

A pre-commit bring-up on the old tree showed derived text with a member heading before the cell text.
The owner documents do not define that heading, so byte 0 is not the cell-text oracle.
The frozen oracle requires the computed cell text in order, as a contiguous UTF-8 sequence.
The same bring-up already measured amplifying live growth above the unchanged ceiling.
The ceiling formula was not edited after that measurement.
Linux procfs and `fcntl` admit the process observer.
This record makes no macOS claim.

## Unresolved Fixture Disposition

This section corrects the ownership wording above and records the remaining fixture lifetime.
The first ownership leg is immediate settlement after spawn.
It is not an injected setup failure.
The label is `observer_guard_settled`.

When bounded `finish` and `Drop` both leave collection unresolved, later product observations on that fixture are `NOT_RUN`.
The fixture path stays with the existing pid and start-token obligation.
Ordinary removal runs only after collection is confirmed.
A failed product predicate does not stop later observations when the child was collected.
Product predicates, the B2 fixture, the allocator windows, and the ceiling are unchanged.

The unresolved-fixture admission uses the same spawn and guard.
It withholds collector status and does not signal or wait.
That seam is not an observed kernel wait failure.
The admission then collects the owned hang child and removes only that admission directory.
A child that remains unresolved after those attempts keeps its directory.
