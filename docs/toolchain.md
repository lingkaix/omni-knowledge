# Toolchain

This guide owns setup and command discovery. The product uses Rust 1.99.0 and Cargo, selected through `mise.toml`.

Use `mise` to manage runtimes and development tools when the project needs them. Add only tools justified by accepted work. Record the versions and setup procedure with the first implementation.

Before adopting a dependency, identify its owner, purpose, license, and maintenance cost. Use the selected package manager's normal dependency and lockfile procedure.

Run `cargo test --features gui`, `cargo clippy --all-targets --features gui -- -D warnings`, and `cargo fmt --all -- --check`.
Run `sh scripts/bundle-macos.sh` for the local macOS app and CLI.
Packaging uses Python 3.9 or newer to collect third-party notices; the app has no Python runtime dependency.
Use `git diff --check` and resolve local documentation links.
Socket mock tests require loopback permission; media tests require ffmpeg and ffprobe.
No successful command proves native layout or real-model accuracy without the corresponding direct observation.

Keep temporary probes under `temp/`. Add a permanent script only when repeated work establishes its need.
