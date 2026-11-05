# Canonical native package-fixture parent

Actual hosted macOS run 41983323983 executed the package target: four passed and 17 failed with `Hostile` during native private-directory binding. The independent public target used a canonical temporary parent and did not share this failure. Raw native evidence is retained.

The producer target's account and shared fixture-directory constructors used `std::env::temp_dir()` directly. On macOS that can include the `/var` alias, while the package owner correctly refuses symlink ancestors. Construct these owned fixtures below the canonical native temporary parent, preserving all original assertions and strict production traversal. A Linux run with a real symlink `TMPDIR` exercises the setup distinction without claiming macOS qualification.

No archive parser, quota, native source check, caller authority or filesystem backend changes. All 21 producer controls and 22 independent public controls remain. New native hosted observation is still required; this ordinary fixture correction does not close full M5, M1 or native-platform acceptance.
