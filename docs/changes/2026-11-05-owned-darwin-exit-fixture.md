# Owned Darwin exit fixture

Hosted run 41988474641 on `9f20a7d` passed Linux and all macOS test targets except the post-fork cache-owner case, which reported `ChildUserspace`. The fixture required both a libproc zombie row and a non-reaping wait result. Darwin can omit that libproc row for an exited child; absence itself must never become proof of exit.

Use the existing kernel `waitid(P_PID, pid, WEXITED | WNOHANG | WNOWAIT)` witness for this owned child: exact `si_pid`, `SIGCHLD`, and `CLD_EXITED`, `CLD_KILLED` or `CLD_DUMPED`. Live, non-child, mismatched and unknown observations do not pass. `WNOWAIT` leaves the child waitable, and the existing `ChildGuard` still actually collects it. This is not group/thread/I/O settlement evidence for arbitrary workers.

A bounded raw pipe acknowledgement keeps the child alive until the parent has captured its actual start token. Removing that handshake reintroduces a token-versus-exit race; replacing the kernel witness with EOF, a marker or missing PID would weaken the oracle. Raw child-side calls avoid adding a Rust thread or allocating I/O machinery after fork. The original 19 assertions, collector and timeout remain.

The producer's Linux cache 16 + owner 3 cases, inventories, checks, fmt and strict Clippy passed. No Apple SDK was installed and no macOS execution is claimed here. Actual native rerun remains required. No production Core, storage, history or authority changes.
