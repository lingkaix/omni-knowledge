# Portable Workspace Transition

Date: 2026-11-04.
Status: M0 preparation; shared crates and service implementation remain pending.
Accepted source: the full-version product direction and amendments accepted on 2026-11-04.

## Intent And Boundaries

Deliver a local folder knowledge layer with desktop, CLI, MCP, and headless Linux faces in one delivery.
Use one Cargo workspace with neutral reusable crates and product-specific service and client crates.
This record describes the transition; product owners in `docs/specs/` define behavior.
The existing root package remains the compatibility subject until independent public-interface tests prove each replacement.
Creating the workspace does not claim that storage, retrieval, service, or desktop migration has occurred.

## Responsibility Map

| Crate | Responsibility | Allowed internal dependencies |
| --- | --- | --- |
| `knowledge-core` | Portable files, mounts, identity, journal, safe writes, history, and packages | No GUI or product service |
| `knowledge-processing` | Extraction, media, transcripts, provider identities, and bounded processing | Core |
| `retrieval-kernel` | Pinned upstream lexical algorithms with generic seam patches | No Omni product types |
| `knowledge-retrieval` | Kernel adaptation, metadata, file-rank fusion, vectors, generations, and coverage | Core, processing interfaces, kernel |
| `omni-service` | Multi-library orchestration, jobs, exposure, and transports | Shared crates |
| `omni-cli` | Headless commands, JSON output, and stdio proxy | Service client |
| `omni-gui` | Native GPUI browsing and preview over service operations | Service client |

Add each crate when its owned contract and first real behavior are available, not as an empty forwarding layer.
Keep default Linux builds free from GUI and macOS framework dependencies.
There is no OpenKit source change in this transition.
Kernel patches must expose generic byte-source, generation/Directory, budget, and shard seams that can later move upstream.
A foreign governance adapter does not make host authority part of the shared crates.

## Preservation And Verification

Independent tests characterize existing public behavior before implementation retirement.

Use the [test lifecycle](../specs/test-strategy.md#test-authority-and-lifecycle) for the distinct proof obligations of behavior-preserving replacement, intended changes, and internal-mechanism retirement.

No test or source module is retired by the initial workspace declaration.
The first declaration has the existing package as its sole member and leaves dependency resolution unchanged.

The targeted gate is Cargo metadata, repository formatting, manifest whitespace, and unchanged package identity/dependency checks.
Compilation and full deterministic baseline are separately accounted integration evidence, not inferred from metadata.
Each later crate landing runs affected tests and Clippy; integrated batches run workspace tests, strict Clippy, and a headless release build.
Record exact final-head observations in the execution evidence.

## Necessity And Ablation

The workspace gives the accepted reusable owners one dependency resolution and one deterministic suite.
Removing it leaves separately resolved packages and prevents a single workspace integration gate.
The initial declaration adds no registry, protocol, runtime process, wrapper, compatibility engine, or duplicate algorithm.
The existing root package is a temporary migration subject, not a second permanent production retrieval engine.
Remove its displaced implementation only after public replacement proof and consumer migration.
