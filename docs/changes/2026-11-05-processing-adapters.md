# Processing Adapters

Date: 2026-11-05. Branch `omni-processing-40e8e01`. Base `d186eac09cbd1b439a389f40746414e46c6c2767`. Account prerequisite `d51cdde487a707cfcda5050655e7bac73943f7c7`.

Mature `extract`, `media`, `epub`, and `runtime` bodies live in `knowledge-processing`.
Root modules re-export the public names, including the incremental XLSX assembler.
Processing calls `knowledge_core::admission::Account::current`. It does not start a second queue or digest.
Live output holdings are returned. Working buffers and settled children are finished. Generated files use `written`.

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| `Account::current` before any effect | One process account has to refuse work when it was not started. | Reads and children proceed with no shared promise. |
| Returned `Holding` for live text | Drop does not release a charge, and finishing on return frees memory the caller still holds. | Text is charged forever, or the charge disappears while the text remains. |
| Admitted file, not a second path open | The host's descriptor is the version. Reopening the path can observe a different file. | A replaced path is parsed as if it were the admitted file. |
| `parser-worker` plus resident-set watchdog | PDF, HTML, and XML DOM allocate before a debit is possible. macOS `RLIMIT_AS` returns `EINVAL`. | Those parsers grow without a ceiling, or a missing sample is treated as success. |

The watchdog kills and waits the child process group. It reads Linux `VmRSS` or macOS `proc_pidinfo` resident size. It does not call `setrlimit`.
Native macOS `textutil` and `proc_pidinfo` were not executed here.
No ablation experiment was required: removing any row above drops the shared account, the live charge, the admitted file, or the opaque-parser stop.
