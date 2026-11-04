# Verification

This document owns evidence quality, oracle and harness admission, measurement method, and environment limits. [Test strategy](specs/test-strategy.md) owns test boundaries, test authority and lifecycle, fixtures, and regression selection. Product contracts own expected behavior; package guides own runnable commands.

## Start With The Expected Result

For a feature or bug fix, normally write the lowest sufficient regression before implementation. Derive the expected result from the accepted behavior. Name the expected failure before running it.

If the failure, environment, or expected result is unknown, run a probe first. For a behavior change without a regression, record why another direct proof is stronger. A setup, permission, or collection failure proves no product defect.

Choose the lowest sufficient public proof boundary under [test strategy](specs/test-strategy.md#choose-a-proof-boundary). A content check does not become product-runtime evidence merely because it passed.

For untested behavior that must remain stable, add a characterization test before refactoring. For a documentation-only change, inspect meaning, references, and consistency. A content check does not need a product runtime test.

## Choose The Lowest Sufficient Observation

- Pure logic: unit or property tests over public behavior.
- Component boundary: integration or contract tests through the actual interface.
- Representation crossing: test encoding, identity, ordering, or recovery on both sides.
- External behavior: an authorized check against the actual service or runtime.
- Text and structure: inspect the diff and check links, commands, and retained criteria.

Use existing test infrastructure first. A disposable probe does not justify a permanent runner. Keep deterministic checks local where their subject permits it.

Tests must not need effects outside the component's responsibility merely for convenience. Treat such a dependency as a boundary finding. A fake can test local logic, but it cannot prove an unobserved remote effect. Use an existing owned observation channel before reconstructing state. A temporary probe may establish a named fact without becoming permanent infrastructure. When a standing gate needs a stable missing observation, return that gap to the subject's owner; do not maintain a parallel source of truth.

## Check The Check

A test oracle defines the expected observation and compares it with the actual observation. Fix that expectation independently of the candidate result.

A deciding oracle needs four properties:

| Property | Required observation | Falsifier |
| --- | --- | --- |
| Prior | Expectations come from an accepted owner independently of the candidate. | Would a different implementation change the expected value? |
| Bounded | A finite enumerated question can reach completion. | Does the instruction ask only to find one more issue? |
| Reproducible | The same observation yields the same verdict for competent evaluators. | Do independent evaluators disagree on the same observation? |
| Re-runnable | Its cost permits rerunning after a relevant change, including an oracle change. | Is rerunning too costly to challenge its verdict? |

Run the applicable falsifier before claiming deterministic proof. State each missing property rather than hiding weakness. A weak oracle informs accountable judgment but cannot establish deterministic proof. Either declare and demote it, or convert the question to a directly observable necessary condition. Add a mechanical proxy only for an observed failure it can detect, not an imagined completeness claim. A judgment about value or simplicity states its authority, evidence, alternatives, and uncertainty; agreement is not reproducibility proof.

For a new or materially changed acceptance harness, demonstrate success and deliberate failure before trusting its verdict. Demonstrate timeout handling if it declares a timeout outcome. Use stable stand-in targets to admit the instrument; this self-check provides no product evidence. Keep a target observable until its deciding observation completes, and use explicit terminal outcomes. A skipped or inapplicable classification needs its own observation and accepted reason. Confirm that the deciding assertion ran for every counted case.

An unchanged admitted harness remains admitted. Changes to its control flow, oracle, terminal outcomes, stand-ins, or deciding observations require renewed admission. An unadmitted harness produces no gate evidence, whether it passes or fails.

Use mutation or fault injection when a claimed detector's power is uncertain. Record the instrument commit or digest, exact check, intervention, mutated path, exercised path, expected result, observations on both sides, and date. Do not count a skipped or unexercised assertion as coverage. Keep mutations off integration branches and verify their removal. When mechanisms back each other up, analyze them together before claiming either is removable.

Do not weaken a check because its environment is inconvenient. Report the missing evidence and use the authorized environment when available. A failing contract-derived check must be corrected at its cause or referred to its owner.

## Bound Environment And Effects

State required platforms and external dependencies before a run. Declare platform exclusions or differences before execution; do not convert a runtime error into a skip. An ordinary deterministic check must not require a container runtime. A check of container behavior is a distinct opt-in real-environment check and must use a real container. A real process, native application, model, or container claim still needs its real subject; a portable fake cannot prove it. Distinguish unsupported environments from product failure. An environment limitation is not a passing result. When two supported environments disagree, retain both results and investigate the difference; the second run does not erase the first.

For real-environment checks, name the target, exact artifact version, observed environment identity, prerequisites, permissions, cleanup, and retained evidence. Use isolated disposable state where practical. Preserve unrelated state on shared systems. Separate machine provisioning, prerequisite assertion, subject bring-up, and product observations when those operations are needed. A recipe or role report does not prove the environment is ready; inspect the actual prerequisites immediately before use. A reusable environment description earns trust through real bring-up, not review alone. Add reusable environment machinery only when repeated local use justifies it.

Never include credentials in test evidence. Report whether cleanup completed and which state remains. A simulation does not establish production behavior.

## Measure Without Retrying The Result

Before a costly run, perform a smoke check of at most 60 seconds on the exact candidate and planned command path. Record its code or artifact identity, inputs, environment, expected observation, and outcome. A different binary or earlier head does not qualify the candidate. If the bounded smoke cannot exercise the path, report that limitation before the expensive run.

Register public-observable thresholds and the measurement protocol before execution. A threshold must observe the user-facing property, not an internal proxy chosen after seeing results. An observer fails open with respect to the subject: observer failure records missing evidence without changing product execution. Fail-open observation never relaxes authorization, confidentiality, containment, or other strict safety controls.

A pre-launch operator defect is excluded from product measurement, corrected, and recorded. Allow at most two evidence re-acquisitions for those pre-launch defects. Excuse a demonstrated gate false positive once, retaining its failed result, cause, correction, and disposition. This exception is not permission to bypass a contract-derived or safety failure. Never retry a measured arm to improve its result. A diagnostic rerun is separately labelled and cannot replace the registered observation or denominator.

Keep the first product failure and later cleanup failures distinct. Settle cleanup on every terminal path and report residual state without replacing the original failure.

## Report Exact Evidence

Run applicable focused lint, type, test, and build checks for the final changed surface. Broaden verification when the affected boundary, release, or engineer requires it.

Record the command, working directory, exact code or artifact identity, result, and material environment limits. Distinguish passed, failed, blocked, and not run. Inspect actual output before reporting a result. A gate on an earlier or pre-rebase head is not a gate on the final candidate. [Change execution](change-execution.md#land-and-check-the-final-head) owns final-head and repository-wide gate timing.

After sufficient checks pass, continue toward completion. Repeat or expand them only when new changes, failures, or unresolved concerns justify the cost.
