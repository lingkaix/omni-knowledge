# Toolchain

This guide owns setup and command discovery. No product runtime or package manager has been selected.

Use `mise` to manage runtimes and development tools when the project needs them. Add only tools justified by accepted work. Record the versions and setup procedure with the first implementation.

Before adopting a dependency, identify its owner, purpose, license, and maintenance cost. Use the selected package manager's normal dependency and lockfile procedure.

There are no build, lint, typecheck, or test commands yet. Do not report them as passed. For this baseline, inspect Markdown, resolve local links, compare retained criteria, and use `git diff --check` for tracked changes.

Keep temporary probes under `temp/`. Add a permanent script only when repeated work establishes its need.
