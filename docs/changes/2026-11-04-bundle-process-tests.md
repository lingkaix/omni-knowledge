# Bundle Process Lifecycle Admission

This record tracks an independent process-test assignment, not a production correction or native product qualification.
The source request is the process-test dispatch of 2026-11-04.
The exclusive source scope is `tests/process/` and this record.

## Owners And Fixed Expectations

The committed `ff1cc2777c5e01b8ca89cfd09da1d48e434f37b5` version of [the scripts guide](../../scripts/README.md) owns the public command path and lifecycle promise.
Its **Test Accounting And Evidence** section requires timeout failure and settlement of command descendants, including interruption.
Its **macOS Bundle** section selects `python3 scripts/gate.py gui` followed by the real `sh scripts/bundle-macos.sh --ci-unsigned`.
Its **Pinned Kernel Dictionary Supply** section requires real build delegation through `python3 scripts/gate.py bundle-build`.
The assignment clarifies that settlement covers actually owned nested tools even when a nested controller creates another session.
[Verification](../verification.md) requires success, deliberate failure, timeout admission, explicit terminal outcomes, and exact cleanup.
[The test strategy](../specs/test-strategy.md) requires real owners at the crossing and allows external tool effects to be doubled.

Before any OLD experiment, the deciding expectation is fixed: after a ready nested tool, timeout or interruption must terminate with failure, leave no live owned descendants, and permit no controlled delayed write after reported settlement.
An early prerequisite refusal is not a lifecycle pass.
Successful and deliberate-failure controls must exercise the selected route.
No exact diagnostic wording, internal function, process-group arrangement, or implementation constant is an oracle.
No existing test is retired.

## Method And Boundaries

Use the actual committed gate and bundle scripts as opaque subjects.
Use only Python standard-library assertions and disposable synthetic state.
Linux stand-ins represent platform prerequisites, Cargo, and supplier effects; their success is instrument admission only.
The actual gate, shell bundle, and nested build controller must execute unchanged.
Keep script hashes, exact commands, readiness, terminal outcomes, delayed-write observations, and cleanup evidence.
No candidate implementation inspection, production change, agent delegation, remote mutation, publication, or push is authorized.
The requested test commit is authorized.

## Instrument And Necessity

[The process-test guide](../../tests/process/README.md) gives the runnable command, mechanism inventory, reasoned ablations, and environment limits.
The implementation is two standard-library Python files: one unittest script and one external-tool fixture.
The original gate, bundle script, nested build delegation, and license collector remain unchanged.
The fixture does not create a detached session for its selected tool or worker.
The real subject creates the lifecycle arrangement under observation.

An atomic readiness barrier establishes the selected nested tool and its ordinary child before intervention.
The driver sends SIGINT only to the exact outer gate PID, or lets the public two-second command deadline expire.
It records outer exit, observes owned descendants, and releases a controlled delayed write before test cleanup.
The deciding assertion compares the live-descendant set and delayed-write marker with an empty set and absence.
A Linux child subreaper preserves observation and cleanup ownership of leaked orphans without settling them on the subject's behalf.
Pidfd-based cleanup follows the deciding observations and reaps only that driver's descendants.
No production process registry or generic runner is added.

## Retained OLD Evidence

OLD is `ff1cc2777c5e01b8ca89cfd09da1d48e434f37b5` from the lead repository.
Its public guide and command help were read; no gate or bundle controller implementation informed expectations.
The original checkout had no `tests/process/` suite.
The public guide's existing one-group self-test does not establish this nested composition and is not retired.

The final command was run from the assigned `omni-knowledge` checkout:

```sh
python3 tests/process/test_bundle_lifecycle.py \
  --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge \
  --subject-ref ff1cc2777c5e01b8ca89cfd09da1d48e434f37b5 \
  --evidence-dir /home/simonx/.cache/openkit-tmp/omni/process-tests/evidence/old-ff1cc27-final
```

The driver exited 1: unittest ran two methods with eight subcases and reported four failures in 12.403 seconds.
The four failures are expected OLD lifecycle failures, not prerequisite failures or claimed product test passes.

| Route | Intervention/control | Outer exit | Deciding observation | Result |
| --- | --- | --- | --- | --- |
| Cargo | Success | 0 | Selected nested route; unchanged real scripts | Admission control passed |
| Cargo | Deliberate tool failure | 1 | Selected nested tool returned 37 | Admission control passed |
| Supplier | Success | 0 | Selected nested route; unchanged real scripts | Admission control passed |
| Supplier | Deliberate tool failure | 1 | Selected nested tool returned 37 | Admission control passed |
| Cargo | Outer timeout | 1 | Ready; two live owned descendants; delayed write present | Expected OLD failure |
| Cargo | SIGINT | 130 | Ready; two live owned descendants; delayed write present | Expected OLD failure |
| Supplier | Outer timeout | 1 | Ready; two live owned descendants; delayed write present | Expected OLD failure |
| Supplier | SIGINT | 130 | Ready; two live owned descendants; delayed write present | Expected OLD failure |

Every lifecycle subcase reached the same settlement assertion.
All eight cases report an empty `cleanup_remaining` set.
Snapshots show the leaked tool and worker in a separate session from the killed bundle command.
All script hashes match their exported originals after execution.
Raw output is retained beside the evidence directory as `old-ff1cc27-final.log`.
The evidence directory contains exact commands, fixture-only environments, tool ancestry, ready barriers, process identities, terminal observations, marker files, and cleanup outcomes.
Synthetic subject copies and artifacts remain intentionally retained there for review.

| Artifact | SHA-256 |
| --- | --- |
| OLD `scripts/gate.py` | `c87d656dfca2cd2bfe7592b4964e755db1e24d8c2ba3e0fc37489221df43ac9b` |
| OLD `scripts/bundle-macos.sh` | `147bb1e659d595bd9a60ade4e52ca4fac03fa15cbc813866de693168da2cda89` |
| Committed-file export archive | `a8080282e9b73f0bd2c894106da034cd36b6bda2f7bee6f39a2d14aa7acf174b` |
| `tests/process/test_bundle_lifecycle.py` | `f85c5268886387724027469a8fb5af0df193a551a5d81c36ec488b9531fa6216` |
| `tests/process/standin.py` | `a46d089793fab3dcb25d353c1e95c4c87f3ce58171a8f155d54a519679e09940` |

### Fixture Development And Oracle Correction

The retained `probe-01` through `probe-07` outputs document construction failures, not lifecycle measurements.
They exposed missing synthetic Git identity, the declared ASR inventory entry, the GUI binary filename, copied license resources, Cargo metadata fields, and the external `plutil` prerequisite.
`probe-08` reached both successful and deliberate-failure controls through the actual selected path.
No subject controller was changed to make those controls work.

`old-ff1cc27-01` first recorded the intended timeout failures and delayed writes on all four lifecycle arms.
Its interruption arms stopped before the settlement assertion because the test additionally required diagnostic words `FAIL` or `TIMEOUT`.
That was an unjustified wording oracle: exit 130 with an explicit interruption message is a public terminal failure, and no owner requires those exact words for interruption.
The independent author removed the wording constraint, not the nonzero-exit, readiness, descendant, or delayed-write obligations.
The original red evidence remains retained.
`old-ff1cc27-02` re-admitted the controls and reached the same failing settlement assertion on all four lifecycle arms.
The final run repeated admission after the marker wait became state-based and docstrings were formatted; all four lifecycle failures remain red.
These construction and re-admission runs are not retries that erase a product measurement.

## Supplied NEW Observation

The supervisor subsequently supplied exact NEW `1764150e44d550a6db6800d8576b1db8ed5ff650` and required unchanged OLD assertions and fixtures.
The existing bundle lifecycle obligation remains in scope despite deferral of GUI redesign.
No NEW implementation was read, and neither Python source file changed before or after this run.
The OLD and NEW `identity.json` files contain identical instrument hashes.

```sh
python3 tests/process/test_bundle_lifecycle.py \
  --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge \
  --subject-ref 1764150 \
  --evidence-dir /home/simonx/.cache/openkit-tmp/omni/process-tests/evidence/new-1764150
```

The NEW driver exited 1: two unittest methods, eight subcases, four failures, 14.543 seconds.
All four supplier subcases passed, including both settlement assertions.
All four Cargo subcases failed at the route assertion `real build delegation absent`, before their deciding assertions.
This is not a complete passing admission and is not evidence of a NEW lifecycle leak.

| NEW observation | Cargo | Supplier |
| --- | --- | --- |
| Success outer exit | 0 | 0 |
| Deliberate-failure outer exit | 1 | 1 |
| Timeout outer exit | 1 | 1 |
| SIGINT outer exit | 130 | 130 |
| Lifecycle readiness | Present on both arms | Present on both arms |
| Live descendants at terminal | None on both arms | None on both arms |
| Delayed writes | Absent on both arms | Absent on both arms |
| Settlement assertion reached | No: blocked by route assertion | Yes: both passed |
| Final cleanup | Empty residual set in every case | Empty residual set in every case |

The retained Cargo ancestry contains the actual outer GUI gate, real unsigned bundle shell, and Cargo tool, but no live `bundle-build` controller.
The actual public Cargo timeout log reports `EXEC bundle-build: cargo build --workspace --locked --release --features gui` before `TIMEOUT: bundle`.
The test's route assertion requires a live ancestor rather than merely the required real invocation.
The owner requires real build delegation, not continued existence of that controller after process replacement.
Thus the independent author's disposition is that the live-ancestor formulation is over-coupled and needs a provenance-observation correction, not a production change to preserve an intermediate process.
The raw observations are consistent with process replacement; no private implementation inspection is needed to identify the unsupported persistence requirement.

The supervisor explicitly required unchanged assertions and fixtures for this NEW comparison.
No assertion was removed, weakened, bypassed, or changed after NEW, and no second NEW run was performed.
The original real-invocation requirement and all nonzero-exit, readiness, descendant, and delayed-write obligations must survive any later authorized instrument correction.
That correction needs re-admission and retained OLD/NEW proof before the full Cargo route can be counted as passing.
Do not integrate this instrument as a complete green gate or reject the production correction solely to satisfy its current live-ancestor assertion.

| NEW artifact | SHA-256 |
| --- | --- |
| `scripts/gate.py` | `b059aca58906458838cbc0f3c719b5707b69e63070ecefe3a62d341ce7a3bcc5` |
| `scripts/bundle-macos.sh` | `147bb1e659d595bd9a60ade4e52ca4fac03fa15cbc813866de693168da2cda89` |
| Committed-file export archive | `1b79ba7860a165bd88c6b4d116a450bb1b25c8ca9125807781569abeef03e3c5` |

Raw NEW output is retained as `new-1764150.log`, beside its complete evidence directory.
All OLD evidence remains intact.

## Authorized Provenance-Observation Correction

A mechanical observer correction was authorized for the live-ancestor coupling documented above: make actual invocation and provenance observation survive process replacement without requiring the intermediate controller to stay alive, while every public command and script remains unchanged and every settlement obligation is retained.

The instrument changed only in its two Python fixture files:

- `standin.py` now records each observed ancestor's immutable start-time identity alongside its PID and argv, and each existing controlled-effect `tool` event now carries the full invocation ancestry observed at that moment.
- The fixture replaces the Cargo-route live-ancestor requirement with an identity binding: the `bundle-build` helper's PID plus start-time identity, observed while it invoked the preceding nested supplier, must appear as the selected Cargo itself or in its ancestry. The supplier route keeps its live-helper observation, and the ownership-crossing, bundle-script, readiness, terminal, zero-live-descendant, no-delayed-write, and cleanup assertions are unchanged.

No production script, manifest, lockfile, existing test, or signing default changed, and no assertion was removed, weakened, skipped, or retuned. A controller that exec-replaces itself with Cargo proves delegation through the retained identity; a controller that would orphan the selected tool still fails the unchanged ownership-crossing assertion.

The corrected instrument was re-admitted on the exact same committed subjects from the lead repository with separate fresh evidence directories and preserved prior red runs:

```sh
python3 tests/process/test_bundle_lifecycle.py \
  --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge \
  --subject-ref ff1cc2777c5e01b8ca89cfd09da1d48e434f37b5 \
  --evidence-dir /home/simonx/AI/temp/backlog-20261015/evidence/omni/process-observer-artifacts/old-ff1cc27-observer

python3 tests/process/test_bundle_lifecycle.py \
  --subject-repo /home/simonx/.cache/openkit-tmp/omni/lead/omni-knowledge \
  --subject-ref 1764150 \
  --evidence-dir /home/simonx/AI/temp/backlog-20261015/evidence/omni/process-observer-artifacts/new-1764150-observer
```

| Observation | OLD `ff1cc27` | NEW `1764150` |
| --- | --- | --- |
| Driver result | Exit 1; two methods, eight subcases, four failures; 11.784 seconds | Exit 0; two methods, eight subcases, all passing; 12.609 seconds |
| Controls | All four passed | All four passed |
| Cargo provenance | Helper identity (PID plus start time) observed at the nested supplier matched the live `bundle-build` ancestor of selected Cargo | The same helper identity matched the selected Cargo process itself: process replacement by exec retained the recorded PID and start time |
| Supplier provenance | Live `bundle-build` helper observed at supplier selection in all cases | Live `bundle-build` helper observed at supplier selection in all cases |
| Settlement arms | Same four intended failures: ready, two live owned descendants, delayed write present, settlement assertion reached | All four passed: ready, zero live descendants, no delayed write, settlement assertion reached |
| Cleanup | Empty residual set in all eight cases | Empty residual set in all eight cases |

Every OLD case passed provenance before failing; the only OLD failures remain the unmodified settlement assertion `terminal gate failure must settle owned descendants before subsequent writes`.
No case failed at provenance, and no Cargo case stopped at the removed coupling.
The NEW exec observation is an immutable process identity: the helper recorded as `python3 scripts/gate.py bundle-build` at supplier selection time carried the same PID and start time as the selected Cargo process.
The runs observed 227 owned process identities in total, none of which remained live afterwards, and no file under the retained earlier evidence root was modified.
This correction proves instrument admission only, not native macOS, GUI, signing, real compilation, licenses, or product test counts, and independent review of the corrected instrument remains an integration-owner obligation.

| Artifact | SHA-256 |
| --- | --- |
| Corrected `tests/process/test_bundle_lifecycle.py` | `03057e443b75d5ba3dafc6843ce2ce4ff369ca2af13b77296291d806a4cd4c73` |
| Corrected `tests/process/standin.py` | `80871c7cfb6e5401daa561ab6c9b1ebd34ef1400a819f8286eb3be9b8bea8aeb` |
| OLD export archive (unchanged subject) | `a8080282e9b73f0bd2c894106da034cd36b6bda2f7bee6f39a2d14aa7acf174b` |
| NEW export archive (unchanged subject) | `1b79ba7860a165bd88c6b4d116a450bb1b25c8ca9125807781569abeef03e3c5` |

## Checks, Scope, And Handoff

Python compilation of both source files passed under Python 3.14.7 on Linux.
The test addition is 402 Python lines and zero production lines; the unbounded test-to-production ratio reflects this tests-only assignment.
The four changed files are the two Python files, their local README, and this record.
No production script, manifest, lockfile, existing test, or signing default changed.
The explicit unsigned route was exercised, but local signing/refusal behavior and native macOS effects are not newly qualified.
Rust compilation, Cargo product suites, and native GUI tests were not run for this Python admission assignment.

The supplied NEW was tested once with unchanged fixtures and assertions; the complete gate remained red for the provenance-observation limitation described above.
The authorized correction, its re-admission on both exact subjects, and the retained OLD/NEW proof are recorded in the section above.
This author cannot independently accept their own consequential test artifact.
No push, deployment, publication, agent delegation, or coordinator message was performed.
