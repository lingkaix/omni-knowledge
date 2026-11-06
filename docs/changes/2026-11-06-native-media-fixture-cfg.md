# Match the Linux media fixture helpers to their existing consumers

Actualdbc87d8 run42243502935: Linux699/full strict/release succeeds; macOS662P0F1 declared ASR opt-in runtime succeeds, then strict fails seven unused/dead-code diagnostics in knowledge-processing/src/media.rs. All functions in this private tests module already have target_os=linux; its /proc-based helpers and private run_bounded seam alone were cfg(test) on every OS.

Gate only that module and seam with all(test,target_os=linux), matching their existing callers. No lint suppression, moved assertion, skipped existing macOS case, production/runner/RSS/deadline/authority/protocol/ceiling change. Actual macOS tools and held-FD processing remain native production paths; Linux fixture execution is not portable proof for those.

The previous fixture status-quote fix passes the complete actual Mac runtime inventory; this is not retrospective identification of its prior unrecorded Waiting reason. Changed-head full Linux gate and actual Mac strict/runtime remain required. This repair-only source excludes private tuple/range/query/statistics work, and is not M4–M6/native/full-product READY.
