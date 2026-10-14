# Verification

This document owns evidence quality and test selection. Product contracts own expected behavior; package guides own runnable commands.

## Start With The Expected Result

For a feature or bug fix, normally write the lowest sufficient regression before implementation. Derive the expected result from the accepted behavior. Name the expected failure before running it.

If the failure, environment, or expected result is unknown, run a probe first. For a behavior change without a regression, record why another direct proof is stronger. A setup, permission, or collection failure proves no product defect.

Use unit tests for business rules and integration tests for API and middleware boundaries where those layers carry the changed behavior. Prefer a focused check to a duplicate test at every layer.

For untested behavior that must remain stable, add a characterization test before refactoring. For a documentation-only change, inspect meaning, references, and consistency. A content check does not need a product runtime test.

## Choose The Lowest Sufficient Observation

- Pure logic: unit or property tests over public behavior.
- Component boundary: integration or contract tests through the actual interface.
- Representation crossing: test encoding, identity, ordering, or recovery on both sides.
- External behavior: an authorized check against the actual service or runtime.
- Text and structure: inspect the diff and check links, commands, and retained criteria.

Use existing test infrastructure first. A disposable probe does not justify a permanent runner. Keep deterministic checks local where their subject permits it.

Tests must not need effects outside the component's responsibility merely for convenience. Treat such a dependency as a boundary finding. A fake can test local logic, but it cannot prove an unobserved remote effect.

## Check The Check

A test oracle defines the expected observation and compares it with the actual observation. Fix that expectation independently of the candidate result.

A deciding check needs a finite question, a repeatable verdict, an affordable rerun, and evidence from the subject it claims to observe. State any missing property. A weak observation can inform judgment but cannot establish deterministic proof.

For a new or materially changed acceptance harness, demonstrate success and deliberate failure before trusting its verdict. Demonstrate timeout handling if it declares a timeout outcome. Confirm that the deciding assertion ran.

Use mutation or fault injection when a claimed detector's power is uncertain. Record the instrument version, intervention, exercised path, expected result, observed result, and date. Do not count a skipped assertion as coverage.

Do not weaken a check because its environment is inconvenient. Report the missing evidence and use the authorized environment when available. A failing contract-derived check must be corrected at its cause or referred to its owner.

## Bound Environment And Effects

State required platforms and external dependencies before a run. Distinguish unsupported environments from product failure. An environment limitation is not a passing result.

For real-environment checks, name the target, artifact version, prerequisites, permissions, cleanup, and retained evidence. Use isolated disposable state where practical. Preserve unrelated state on shared systems.

Never include credentials in test evidence. Report whether cleanup completed and which state remains. A simulation does not establish production behavior.

## Report Exact Evidence

Run applicable focused lint, type, test, and build checks for the final changed surface. Broaden verification when the affected boundary, release, or engineer requires it.

Record the command, working directory, result, and material environment limits. Distinguish passed, failed, blocked, and not run. Inspect actual output before reporting a result.

After sufficient checks pass, continue toward completion. Repeat or expand them only when new changes, failures, or unresolved concerns justify the cost.
