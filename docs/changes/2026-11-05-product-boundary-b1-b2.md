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
The stand-ins cover prompt refusal, a lying success prefix with a canary, early EOF, and a hang that must time out.
Cleanup has to reap the started pid, and a leftover process fails the controller case.
The allocator counter delegates to the system allocator and is admitted with an 8 MiB hold and a 4 KiB hold.
Those controller cases are not MCP or extractor results.
The stand-in is the same `product_boundaries` test with `OMNI_PRODUCT_BOUND_STANDIN` set to `refuse`, `echo`, `early-eof`, or `hang`.
That child exits in the named observer role and is counted only as a controller case.
With the variable unset, the test runs admission and the product oracle.
This target has no ignored test and no empty test.
The earlier ignored helper remains only on the preserved receipt commit.

## Case Count

The controller count is 6: CRC-32 vector, four observer stand-ins, and one allocator case that runs both windows.
The product count is 10: two XLSX cases and eight MCP cases.
One of the XLSX cases and the unterminated MCP case are the deciding regressions.
The other eight product cases preserve ordinary behavior.

## Proof Boundary

A pre-commit bring-up on the old tree showed derived text with a member heading before the cell text.
The owner documents do not define that heading, so byte 0 is not the cell-text oracle.
The frozen oracle requires the computed cell text in order, as a contiguous UTF-8 sequence.
The same bring-up already measured amplifying live growth above the unchanged ceiling.
The ceiling formula was not edited after that measurement.
Linux procfs and `fcntl` admit the process observer.
This record makes no macOS claim.
