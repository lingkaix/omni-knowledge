# Processing Adapters

Date: 2026-11-05. Branch `omni-processing-40e8e01`. Base `d186eac09cbd1b439a389f40746414e46c6c2767`.

Mature `extract`, `media`, `epub`, and `runtime` bodies moved into `knowledge-processing`.
Root modules re-export the public names, including the incremental XLSX assembler from that base.
`knowledge_core::admission::Account` is committed on the resource-admission branch as `d51cdde` and is not in this branch's `knowledge-core`. Admitted calls take `SharedAccount` and `admission::reservation` so the host can reserve on that account. This crate does not start a second queue or digest.

| Mechanism | Present need | What removal loses |
| --- | --- | --- |
| Move into `knowledge-processing` | Retrieval and service must call the mature parsers without the root GUI/MCP crate. | Callers stay tied to the root crate or a second parser appears. |
| `SharedAccount` charge before each effect | One process account has to see input, work, output, live, and storage before those effects. | Effects run on the legacy ceilings only, or a private pool diverges from Core. |
| Child resident-set watchdog | macOS `RLIMIT_AS` returns `EINVAL`. A sample that does not kill the child is not a ceiling. | An owned ffmpeg, whisper, or textutil child can pass the admitted promise and still be reported as success. |

The watchdog kills and waits the child process group. It reads Linux `VmRSS` or macOS `proc_pidinfo` resident size. It does not call `setrlimit`.
Native macOS `textutil` and `proc_pidinfo` were not executed here.
No ablation experiment was required: removing any row above drops a required caller, charge, or stop.
