# One Bundle Process-Lifecycle Owner

Date: 2026-11-04.
Status: source correction; independent nested-path admission and final composed gates pending.
Owner: [script lifecycle](../../scripts/README.md#test-accounting-and-evidence) and [verification](../verification.md).

## Actual Defect

The GUI gate starts the real bundle shell in a new process group.
The shell's internal `gate.py bundle-build` formerly started Cargo in another new session/group.
Killing only the outer group could leave Cargo and its descendants writing the target after reported timeout/interruption.
The nested default build deadline also did not make a shorter outer deadline effective for that escaped group.
An equivalent escape existed during the internal dictionary supplier call.
The earlier one-group admission was insufficient; this is the independent S2-06 finding, not a claimed native Mac observation.

## Smallest Correction And Ablation

Retain the existing outer lifecycle owner rather than add a process-tree manager, watcher, registry, or cleanup daemon.
Only internal bundle preparation uses the existing command runner without creating a session; it keeps the supplier inside the bundle group and settles its direct child.
The known supplier performs bounded file/hash/archive work and starts no processes of its own.
After preparation, the internal helper replaces itself with Cargo using `exec`, inheriting the bundle group and prepared environment.
The outer GUI gate therefore owns the effective command deadline and settlement for this entire concrete path.
Other gate commands retain their existing isolated group, deadline, evidence, failure and interruption behavior.
Local signed bundling and explicit CI-unsigned/refusal policy are unchanged; standalone local builds retain foreground behavior.

The ownership option is necessary only to avoid detaching the known nested supplier.
Removing it leaves that supplier outside the outer group; retaining it but restoring a nested new-session Cargo runner leaves the build escape.
Removing the nested Cargo supervisor via `exec` is smaller than forwarding cancellation through two competing supervisors.
No general claim is made about hostile tools that deliberately daemonize into unrelated sessions.

## Required Observations

The independent author must exercise actual GUI dispatch, actual bundle shell and actual build delegation, with controlled external prerequisites/tools.
Establish the nested supplier/Cargo readiness barrier, then exercise a shorter outer deadline and interruption.
Observe terminal failure, settled owned descendants and absence of their delayed writes, with a success/failure control preventing an early prerequisite error from looking like cleanup proof.
Retain expected OLD failure and NEW results at exact committed script identities.
These observations admit the instrument only; native GUI build, signing, app contents and hosted Mac proof remain separate.
