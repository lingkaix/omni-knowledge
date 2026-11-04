# Contributing To Omni Knowledge

This guide owns contribution and commit procedure. Start with [AGENTS.md](AGENTS.md) and use the [documentation map](docs/README.md) to find the relevant owner.

## Prepare The Change

Read the request, accepted contract, affected source, and local guide. Inspect Git status before editing. Preserve unrelated work.

Follow the [toolchain guide](docs/toolchain.md) for setup and commands. Read installation scripts before running them. Identify changes to the host, dependencies, hooks, or remote services before applying them.

Use [change execution](docs/change-execution.md) when the work needs a durable plan or coordination. Use [verification](docs/verification.md) to choose evidence.

## Document Code

Document changed types, interfaces, structs, classes, functions, methods, and test functions, including private elements. Document modules where the language convention requires it.

| Language | Form |
| --- | --- |
| Go | GoDoc. Start with the declared name and describe parameters, returns, and errors. |
| JavaScript and TypeScript | JSDoc for purpose, parameters, returns, errors, and relevant examples. |
| Python | PEP 257 docstrings for modules, classes, and functions. |
| Rust | RustDoc for types, traits, functions, arguments, returns, and errors. |
| Other languages | Use that language's standard documentation convention. |

Describe non-obvious decisions and constraints. Keep code and documentation in English. Use [technical writing](docs/writing.md) for prose.

## Verify And Review

Inspect the final diff and affected consumers. Run the checks required by the changed surface under [toolchain](docs/toolchain.md#checks-and-packaging). Use [test authority and lifecycle](docs/specs/test-strategy.md#test-authority-and-lifecycle) before changing failing tests. Report exact results and checks that could not run. Before an authorized push, gate the final integrated or rebased HEAD, including the applicable repository-wide check.

Use independent review for governing documents and consequential results. The reviewer checks actual artifacts, retained criteria, safety, correctness, simplicity, and evidence. Resolve in-scope findings before claiming completion.

## Commit Authorized Work

Commit only with user authorization. Stage exact paths and inspect the staged diff. Keep each commit coherent and reviewable. There is no required number of commits per feature.

Use Conventional Commits 1.0.0: `<type>[optional scope]: <description>`. Supported types are `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`, and `revert`.

Mark a breaking change with `!` after the type or scope, or with a `BREAKING CHANGE:` footer. Explain its effect and any required migration.

Example: `docs: establish engineering principles and workflow`.

Run the applicable checks before committing. Hook success does not prove every obligation. If a hook fails, inspect the cause; do not bypass a contract-derived failure to obtain a commit.

A pull request description states the problem, resulting behavior, verification, and unresolved risk. Keep its title consistent with the commit convention. Push, publication, release, and deployment need authorization for their effects.
