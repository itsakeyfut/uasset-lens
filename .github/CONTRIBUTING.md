# Contributing to uasset-lens

Thank you for your interest in contributing! All forms of contribution are welcome —
bug reports, documentation improvements, new lint rules, and parser improvements alike.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Prerequisites](#prerequisites)
- [Ways to Contribute](#ways-to-contribute)
- [Reporting Bugs](#reporting-bugs)
- [Feature Requests](#feature-requests)
- [Pull Requests](#pull-requests)
- [Commit Messages](#commit-messages)
- [Code Style](#code-style)
- [Testing](#testing)

---

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Prerequisites

**Rust toolchain** (MSRV: 1.96.0)

```sh
rustup toolchain install stable
rustup component add rustfmt clippy
```

No external dependencies are required — `rusqlite` is bundled statically and the
`.uasset` binary parser is hand-written with no FFI or C libraries.

**Verify the build:**

```sh
cargo build --workspace
cargo test --workspace
```

---

## Ways to Contribute

- **Bug reports** — open a GitHub Issue using the Bug Report template
- **Feature requests** — open a GitHub Issue using the Feature Request template
- **Documentation** — fix typos, improve examples, expand `docs/`
- **New lint rules** — add a rule to `crates/lint-engine/src/rules/`
- **Parser improvements** — extend `crates/scanner/src/parser/` for new UE5 asset types
- **Test fixtures** — add real `.uasset` files to `tests/fixtures/` for asset types not yet covered

---

## Reporting Bugs

Use the Bug Report issue template and include:

- The `uasset-lens` version and OS/architecture
- Steps to reproduce (a minimal `.uasset` file if applicable)
- Expected vs. actual behavior, including full error output

---

## Feature Requests

Use the Feature Request issue template. Describe the UE5 workflow problem you are solving
and how you envision the solution fitting into the existing command set.

---

## Pull Requests

1. Fork the repository and create a branch from `main`.
2. Make your changes following the [Code Style](#code-style) guidelines.
3. Add tests covering your change (see [Testing](#testing)).
4. Run the full check suite locally before pushing:
   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
5. Open a PR against `main` and fill in all sections of the pull request template.

---

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>
```

| Type | When to use |
|---|---|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code change with no behavior change |
| `test` | Test additions or fixes |
| `docs` | Documentation only |
| `chore` | Build, tooling, or dependency changes |
| `perf` | Performance improvement |

**Rules:**
- One line only — no body, no footer
- English, present tense, imperative mood ("add" not "added")

**Examples:**
```
feat(scanner): parse SoftObjectPath array from DataTable rows
fix(cli): handle missing scan DB with exit code 2
test(lint-engine): add naming prefix rule for NiagaraEmitter
```

---

## Code Style

This project follows the rules documented in `docs/rules/`:

| File | Covers |
|---|---|
| `docs/rules/rust.md` | Error handling, parallelism, path handling, database |
| `docs/rules/binary-parser.md` | `.uasset` parser conventions (byteorder, endianness) |
| `docs/rules/cli-output.md` | stdout/stderr separation, JSON output, exit codes |
| `docs/rules/perf.md` | Performance targets and memory limits |
| `docs/rules/test.md` | Test naming convention and coverage expectations |

**Key points:**
- `unwrap()` / `expect()` are only permitted inside `#[cfg(test)]` blocks
- Library crates use `thiserror`; `cli` and `apps` use `anyhow`
- Comments explain **why**, never **what**
- Test names follow the `feature_should_expected_result` pattern

---

## Testing

Run all tests:
```sh
cargo test --workspace
```

Run a specific crate:
```sh
cargo test -p lint-engine
cargo test -p scanner
```

Integration tests for the scanner use real `.uasset` fixture files in `tests/fixtures/`.
If you add support for a new asset type, add a corresponding fixture and integration test.

---

## License

By contributing to uasset-lens, you agree that your contributions will be licensed
under the terms of both the [MIT License](../LICENSE-MIT) and the
[Apache License 2.0](../LICENSE-APACHE).
