# Keep cache producers and readers on the same selected namespace

The earlier fresh-init volume repair selected an OS cache for legacy index builds, while a fresh legacy query process and status still selected the temporary cache.
This caused MCP search and status failures on the pre-dependency-update commit, even when both directories were on the same device.
A single side-effect-free locator now supplies rootless and root-aware startup; reads and builds pass the known library root so the existing device fallback agrees across processes.
An explicit cache stays authoritative, and the existing root-overlap checks still run before directory creation.
The sole process account is retained after startup; no operation switches its account or migrates an existing cache.
Status reads the actual account-held generated-directory descriptor and retains its no-follow traversal, ownership, mode and volume checks.
Cold status still refuses to initialize an account or create a cache.
Native source selection qualifies cache exclusion before creation or reuse while allowing originals on another device; the legacy in-home publication device rule is not applied to native source roots.
The status test locating helper observes that same actual descriptor identity before fault injection; all status, corruption, contention and source-currentness assertions remain intact.
Fresh-init tests also check cold BM25 on both real filesystem devices, preserving the original file search and alias-refusal cases.
This repair changes cache selection consistency, not authority, resource limits or the native tuple engine.
