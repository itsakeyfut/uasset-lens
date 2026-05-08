# README and OSS publication preparation

## Summary

Write the English README and verify the crate is ready for publication on crates.io.
Complete when `cargo publish --dry-run` passes and the README covers all commands
with output examples.

## Design Notes

**README sections:**

1. **Project description** — "Clippy for Unreal Assets": what it does, the problem it solves
2. **Installation** — `cargo install uasset-lens` and GitHub Releases binary download
3. **Quick start** — shortest path from zero to useful output (scan → impact)
4. **Command reference** — one subsection per command with options table and output example
5. **`.uasset-lens.toml` configuration** — the `[scan] exclude_paths` example
6. **System requirements** — UE 5.1+, Windows/macOS/Linux, supported file types (`.uasset`/`.umap`, not IoStore)
7. **Contributing** — brief note pointing to issues

**Publication checklist:**
- License file: `LICENSE-MIT` or dual `LICENSE-MIT` + `LICENSE-APACHE`
- `[package]` in root `Cargo.toml`: `description`, `homepage`, `repository`, `license`, `keywords`, `categories`
- `cargo publish --dry-run` passes without errors

## Requirements

- [ ] Write `README.md` in English with all 7 sections listed above
- [ ] Add a `LICENSE-MIT` file (or dual MIT/Apache-2.0)
- [ ] Add `description`, `repository`, `license`, `keywords`, `categories` to `Cargo.toml` `[package]`
- [ ] `cargo publish --dry-run` passes for `uasset-lens-cli` (or the published crate)
- [ ] README mentions the UE version support limitation (UE5.1+, no IoStore)

## Related

- Closes Phase 3
- Docs: `docs/roadmap/phase3/ROADMAP.md` — Task 6
