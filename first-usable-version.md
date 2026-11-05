# First Usable Version

Status: IN PROGRESS.
This page is not READY and is not a compliance, notarization, or milestone-acceptance claim.

The headless Linux and macOS archives are the current delivery gate.
GUI milestone M7 is deferred.
A GPUI or NSH launch is not required for the headless gate and is not treated as a passed product qualification.

## Checklist

| Item | State |
| --- | --- |
| Trunk contains integrated core, cache, wire, local, jobs, public tests, B1/B2, and the kernel self-VM diagnostic at `faab733` | Present in this branch base. Presence is not acceptance. |
| Headless archive selects Cargo workspace product binaries and does not invent a worker | `omni` is installed. `parser-worker` is installed only when Cargo metadata and the parent lookup both name it; this trunk has neither, so no parser helper is invented. `omni-fsevents` is installed beside `omni` on apple-darwin only. `retrieval-kernel` stays out. A Linux binary is not FSEvents proof. Native macOS execution of that helper remains unobserved. |
| Linux x86_64 headless archive, notices, and CLI smoke | Local observation is recorded in the M8 change record. Hosted CI is not run by this lane. |
| macOS arm64 headless archive | Defined in the workflow. Not executed here. Native macOS remains pending. |
| Seven kernel faces and the macOS RSS watchdog typed error | Remain in the headless test gate. This change does not ignore, exclude, or retune them. The AS ceiling is unchanged. |
| Notice gaps adobe-cmap-parser, htmlescape, pdf-extract, and type1-encoding-parser | Explicit. Cached source or fallback evidence only. No invented license text. |
| Unsigned macOS `.app`, Developer ID, notarization, and Gatekeeper | Deferred with M7. Local `scripts/bundle-macos.sh` remains. |
| Final hosted Linux and macOS CI | Pending. This lane does not dispatch or push. |
| Milestone authority, durability review, hostile ZIP admission, and actual service entry points | Pending. Not claimed by a successful archive script. |
| Linux arm64 | Not configured. Unqualified. |

## Setup, Recovery, And Migration

Install Rust 1.99.0 from `mise.toml`, Python 3.11 or newer for the dictionary supplier, and system ffmpeg and ffprobe.
Prepare the pinned dictionary cache before Cargo, as [the toolchain](docs/toolchain.md) describes.
Build with `cargo build --workspace --release --no-default-features --locked --offline`.
Stage an archive only after that build:

```sh
python3 -B scripts/bundle-headless.py --target x86_64-unknown-linux-gnu --stage dist/linux-x86_64
```

The installed CLI does not need Python.
Initialization, library migration, and recovery remain owned by [storage](docs/specs/storage.md) and the root guide.
This packaging change does not install a daemon or edit host configuration.
Do not point the CLI at a private library from CI.
