# Bundle Lifecycle Admission

This directory tests the public script composition, not the native product.
Read [the script owner](../../scripts/README.md), [verification](../../docs/verification.md), and [the change record](../../docs/changes/2026-11-04-bundle-process-tests.md) first.

## Run Against A Committed Subject

Use Linux with procfs, child-subreaper support, pidfds, Git, a POSIX shell, and Python 3.12 or newer.
No Rust compiler, macOS installation, model, network service, or third-party Python package is needed.
Select a new evidence directory for each run.

```sh
python3 tests/process/test_bundle_lifecycle.py \
  --subject-repo /absolute/path/to/subject-repository \
  --subject-ref COMMITTED_REVISION \
  --evidence-dir /absolute/path/to/new-evidence-directory
```

The default command runs two ordinary unittest methods with eight subcases.
Four controls distinguish nested Cargo and supplier success from deliberate failure.
Four regressions cross those routes with a two-second outer command timeout or SIGINT after readiness.
A missing prerequisite, missing selected route, or missing ready barrier fails the check; it never becomes a skip or cleaned-up success.
The exact OLD run is expected to exit 1; use a separate committed NEW subject to test a correction.

## Real Subjects And Synthetic Effects

The test exports the committed scripts, build manifests, license resources, and brand assets through `git archive`.
It never reads controller implementations for an oracle, imports their private functions, rewrites them, or replaces their delegation.
Each case executes the actual `python3 scripts/gate.py gui`, real `sh scripts/bundle-macos.sh --ci-unsigned`, and real nested `python3 scripts/gate.py bundle-build`.
Owned ancestor observations prove that the selected external tool came through all three invocations: the supplier route observes the live `bundle-build` helper at its own selection, and the Cargo route binds the helper's immutable PID plus start-time identity, recorded while it invoked the preceding nested supplier, to the selected Cargo itself or its ancestry.
The test requires the script hashes to remain unchanged.

`standin.py` supplies external Cargo and dictionary-supplier effects, platform prerequisites, and synthetic CLI artifacts.
A platform-only `sitecustomize.py` reports Darwin ARM64 to the actual gate; it does not alter process APIs or controller code.
The synthetic Git command reports the export's real committed identity because the archive has no `.git` directory.
The driver obtains that identity with real Git before installing any stand-in.
Fake libtest output includes the owner's declared real-ASR exclusion only to reach bundling; these are not executed product tests.
The actual license collector and bundler run with synthetic Cargo metadata and files.
Their successful outputs do not qualify licenses, compilation, macOS, GUI behavior, native signing, or artifact health.

The selected nested tool starts an ordinary child without detaching it.
The child declares readiness and waits for a release file before writing a synthetic marker.
The driver records outer termination and all live owned descendants, then releases the possible late writer.
The deciding assertion requires both zero live owned descendants at termination and no delayed write before the bounded observation ends.
Zombies cannot write and are excluded from the live set, but final cleanup still reaps them.
The shell exit status supplies the public terminal success/failure result; diagnostic wording is not an oracle.

## Necessity And Reasoned Ablation

| Mechanism | Present need | What fails without it |
| --- | --- | --- |
| Actual gate, bundle, and build delegation | Exercise the real lifecycle ownership crossing | A copied supervisor or direct private-function test can miss the nested session leak |
| Minimal external stand-ins | Reach the selected path on Linux without real compilation or personal state | A native prerequisite refusal can prevent every deciding observation |
| Two selected tool routes | Supplier preparation and Cargo execution are distinct external invocations within the real build delegation | One route does not establish ownership settlement for the other |
| Success and deliberate-failure controls | Admit the selected path and distinguish terminal outcomes | An always-failing prerequisite can resemble successful cleanup |
| Atomic ready barrier and ordinary child | Establish the actual nested long-running tool and descendant before intervention | An early timeout or signal can avoid the ownership crossing |
| Post-terminal release and marker | Demonstrate remaining write capability rather than merely a PID observation | An apparent cleanup can still allow delayed effects |
| Linux subreaper and procfs identities | Observe and retain only this fixture's descendants after OLD orphans a session | Parent ancestry disappears after the outer gate exits, which can hide leaks or prevent cleanup |
| Pidfd cleanup after observations | Signal exact owned processes without PID-reuse or broad-name hazards | Cleanup can affect another process or mask the failed settlement before observation |
| Immutable export and retained evidence | Bind a verdict to exact subject and instrument bytes | A green summary cannot establish which composition ran |

There is no added runner, plugin system, process registry, production supervisor, or duplicate assertion/count framework.
The subreaper is only an observer and final cleanup owner; it sends no signal during the subject's settling interval.
Readiness uses state-based polling, not a startup sleep.
Each case has a 20-second driver deadline, each synthetic worker has a 30-second fallback deadline, and final cleanup has a three-second deadline.
The post-terminal marker window is at most half a second; the separate live-descendant assertion prevents that bounded window from excusing a still-running writer.
These fixture bounds are not product latency requirements.

## Evidence And Limits

`identity.json` retains the exact export command, commit, archive digest, script hashes, interpreter, instrument command, and instrument hashes.
Each case retains `outer.log`, actual gate evidence, fixture events, ready ancestry, process identities, and `observations.json`.
The observation file distinguishes the deciding result from final cleanup and marks whether the settlement assertion ran.
Only synthetic paths and the explicitly constructed child environment are recorded; no user environment or credential file is read.
Evidence directories are intentionally retained for review, including synthetic subject copies and artifacts.
A new directory is required, so later runs cannot erase previous failures.

Final cleanup uses only processes created by that test driver, even when OLD leaks them into separate sessions.
No process-name kill, unrelated process signal, private library, remote write, or publication is involved.
The selected path is explicitly CI unsigned, and production scripts remain unchanged.
This suite does not replace local signing/refusal checks or native release acceptance.
NEW must run with the same deciding assertion after an independently supplied committed correction; OLD failure alone is not correction evidence.

### Process-Replacement Provenance Observation

The original instrument required a live `bundle-build` ancestor of the selected Cargo tool.
Run unchanged against supplied NEW `1764150`, it blocked all four Cargo cases at that assertion because NEW replaces its build-controller process with Cargo by exec; see the change record for complete results and artifact identities.
Requiring that controller to remain alive is not an owner obligation: the owner requires the real build-delegation invocation, not retention of the intermediate controller process.
Under authorization, the corrected instrument records the helper's immutable PID plus start-time identity at the preceding nested supplier invocation and binds the selected Cargo to that identity in itself or its ancestry; the supplier route keeps its live-helper observation, and every other assertion stays unchanged.
Re-admission on exact OLD `ff1cc27` reproduced the same four intended settlement failures after provenance succeeded, and exact NEW `1764150` passed all eight subcases with zero live descendants, no delayed writes, and empty cleanup sets.
This proves instrument admission only, not native macOS, GUI, signing, real compilation, licenses, or product test counts.
