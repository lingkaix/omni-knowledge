# Native parent-death guard and canonical history fixtures

Hosted macOS job 41966633737 failed to link the cache consumer: undefined `__errno_location` and `prctl`. Two history setups still created a non-canonical temporary root.
Errno now reads the process last OS error. Linux children still arm `prctl` `PR_SET_PDEATHSIG`. The parent `ChildGuard` still collects the real child pid; self-kill is not a reap.
macOS children record the parent pid before the race. A C thread started only after fork self-terminates if that parent is gone. No Rust thread is started before fork.
Signing refusal and killed-process recovery use the same canonical temporary helper as `Fixture::new`. All 13 history assertions stay. Strict path validation is unchanged.
Linux cache and history gates were run in the builder checkout; an initial parallel `Busy` result was retained before its passing rerun. Integration now captures the expected parent identity before `fork`, so reparenting before the child starts its guard cannot be mistaken for the original parent.
The first integrated workspace gate retained a history fixture `Deadline`: one 60-second claim had covered the entire independent invalid-request matrix and later positive commit. Each independent rejected operation now gets its own disposable native fixture and live claim; the positive config/native-binding checks start with a fresh fixture. The 60-second ceiling, all assertions, and production code are unchanged. Raw first-gate evidence is retained.
macOS was not run locally; hosted native verification remains required.
