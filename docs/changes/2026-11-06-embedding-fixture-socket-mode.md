# Normalize the owned embedding fixture's accepted socket mode

Public012 Linux passed 694 tests. Its actual macOS job42181940263 failed one legacy embedding test with `/health` connection reset (errno54); strict checks were not reached. That raw failure is retained.

The finite joined `tests/embedding.rs::MockService` had a nonblocking listener, but read accepted streams as timeout-based blocking streams. A read error was treated as EOF and even an incomplete header could produce a reply/close. Accepted flags are platform-dependent.

A concrete one-test OLD control forces the inherited nonblocking descriptor state, supplies only part of a real HTTP header, and observes an incorrect premature response (`Ok(1)`). This is not a sleep-as-readiness control: success requires withholding a response until the remaining header arrives, then receiving the actual complete response.

NEW explicitly sets the accepted stream blocking before its existing two-second read timeout, refuses incomplete headers/bodies, and retains the actual stop/join lifetime. All 14 embedding tests pass locally. No application provider/client, model identity, HTTP protocol, retry, hostile ceiling or assertion was weakened.

The control establishes this fixture defect and its repair. It does **not** retroactively establish the exact descriptor flags in the old hosted failure, prove actual macOS repair, load WeMM weights or prove model quality. A changed-head actual macOS run is still required.
