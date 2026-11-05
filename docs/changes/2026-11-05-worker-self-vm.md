# Worker self-VM summary

This record owns one numeric self-observation at the retrieval kernel's existing resource install.
It does not change AS 2 GiB, file-size 512 MiB, the CPU allowance, their order, or fail-closed startup.
The root product does not call this executable. The observation does not protect the product and does not close M0.

## Necessity

The macOS face failure is still `setrlimit` returning an error before input.
DIAG-01 names the resource, errno, and inherited limits.
It does not record the virtual size or resident size of the process that made the call.
Those two sizes are the facts needed to interpret a later Darwin `EINVAL` against the unchanged ceiling.
A separate probe would measure another process. This helper reads only the installing process, once, at that call.

## Observation

Linux reads at most 4096 bytes of `/proc/self/status` and copies `VmSize` and `VmRSS` with the unit token from that file.
proc(5) defines that `kB` token as 1024-byte units.
macOS calls `task_info` with `MACH_TASK_BASIC_INFO` on `mach_task_self`.
`virtual_size` and `resident_size` are `mach_vm_size_t` bytes in the locked libc 0.2.190 layout and in `task_info.h`.
`kern_return_t` is reported when it is not `KERN_SUCCESS` (0).
The count must cover both 8-byte fields before either value is read.
No map list, address, path, other pid, environment, or RSS alias is read.
A missing value stays unavailable. No number is invented.

Success writes one `self_vm point=installed` line to stderr after the three limits return.
Failure appends one `self_vm point=getrlimit` or `point=setrlimit` clause to the existing `unavailable` error and does not also write the success line.
Stdout and the service protocol stay unchanged.
The printed sizes are not enforcement and are not a new ceiling.

## Proof boundary

Linux runs the existing seven executable faces, strict Clippy for this package, and one owned `--version` execution that prints the summary.
macOS source is present and was not executed. Native macOS measurement remains unobserved until an authorized runner executes this executable.
No SDK, host configuration, or push is used to manufacture that run.
