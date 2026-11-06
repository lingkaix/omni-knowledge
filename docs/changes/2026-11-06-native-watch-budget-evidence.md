# Native watch Budget evidence

Actual public 0a66933 macOS completed with one failing runtime case: the limited max_files=2
watch in tracking_native.rs returned Budget during construction. Its first 16-MiB watch and
the original History/kernel resource cases passed. The external observer sampled no FSEvents
helper; this does not diagnose a memory overrun, helper exit, or expiry.

With existing OMNI_NATIVE_TRACE, the actual foreground lease now reports its owned helper's
exit code, or the actual missing/over-limit RSS sample plus a second exact owned exit
observation. Limits, work/exclusion counts and whether READY was actually captured are numeric
facts only. No source paths/content are logged. Budget/expiry/currentness/unknown-work
classification and complete prepayment remain unchanged; missing samples are not readiness.
No fixture threshold, cap or assertion was raised, skipped or weakened.

This adds deciding branch evidence for a subsequent native run, not a repair or native proof.
The existing two-exclusion fixture alone does not establish a policy-count overflow; that
hypothesis is not an accepted cause. Native changed-head outcomes remain separately required.
