# `apps/uasset-lens-cli` — release binary entrypoint

## Summary

Create the `uasset-lens-cli` binary crate with a minimal `main.rs` that delegates to
`cli::run()` and configure the release profile.
Complete when `cargo build --release` produces a working `uasset-lens` binary.

## Design Notes

**`src/main.rs`:**

```rust
fn main() {
    std::process::exit(cli::run());
}
```

**`Cargo.toml`:** depends only on the `cli` crate.

**Release profile** (add to root `Cargo.toml`):

```toml
[profile.release]
opt-level = 3
lto       = "thin"
strip     = true
```

`strip = true` removes debug symbols from the binary, reducing size significantly on Windows.

## Requirements

- [ ] Create `apps/uasset-lens-cli/src/main.rs` calling `std::process::exit(cli::run())`
- [ ] Create `apps/uasset-lens-cli/Cargo.toml` with `cli` as the only dependency
- [ ] Add `[profile.release]` to root `Cargo.toml` with `opt-level = 3`, `lto = "thin"`, `strip = true`
- [ ] `cargo build --release` completes without errors
- [ ] `./target/release/uasset-lens --help` exits with code 0 and lists all commands

## Related

- Depends on: #12 (cli::run())
- Closes Phase 1
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 6
