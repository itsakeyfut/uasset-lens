# Initialize Cargo Workspace

## Summary

Create the root Cargo workspace configuration and the skeleton for all 5 Phase 1 crates,
along with the test fixture directory structure.
This issue is complete when `cargo check --workspace` passes.

## Design Notes

**Workspace layout (Phase 1 crates only — do not create future-phase crates yet):**

```
uasset-lens/
├─ Cargo.toml
├─ crates/
│   ├─ shared/            # src/lib.rs (empty)
│   ├─ scanner/           # src/lib.rs (empty)
│   ├─ asset-db/          # src/lib.rs (empty)
│   └─ cli/               # src/lib.rs (empty)
├─ apps/
│   └─ uasset-lens-cli/   # src/main.rs (fn main() {} only)
└─ tests/
    └─ fixtures/
        ├─ valid/
        ├─ invalid/
        └─ README.md
```

**`[workspace.dependencies]` — centralize all dependency versions here; each crate inherits with `{ workspace = true }`:**

| Crate | Features |
|---|---|
| `thiserror` | — |
| `serde` | `["derive"]` |
| `byteorder` | — |
| `rayon` | — |
| `walkdir` | — |
| `rusqlite` | `["bundled"]` |
| `clap` | `["derive"]` |
| `anyhow` | — |
| `tracing` | — |
| `petgraph` | — |
| `serde_json` | — |
| `toml` | — |

**`.gitattributes` — disable diff/eol conversion for binary test fixtures:**

```
tests/fixtures/**/*.uasset binary
tests/fixtures/**/*.umap   binary
tests/fixtures/**/*.bin    binary
```

**Verification:**

```powershell
cargo check --workspace
```

## Requirements

- [ ] Create root `Cargo.toml` with `resolver = "2"`, `edition = "2021"`, and all 5 Phase 1 workspace members declared
- [ ] Define all dependency versions under `[workspace.dependencies]`
- [ ] Create `crates/shared/Cargo.toml` and `src/lib.rs` (empty)
- [ ] Create `crates/scanner/Cargo.toml` and `src/lib.rs` (empty)
- [ ] Create `crates/asset-db/Cargo.toml` and `src/lib.rs` (empty)
- [ ] Create `crates/cli/Cargo.toml` and `src/lib.rs` (empty)
- [ ] Create `apps/uasset-lens-cli/Cargo.toml` and `src/main.rs` (`fn main() {}` only)
- [ ] Add `target/` and `.uasset-lens/` to `.gitignore`
- [ ] Create `tests/fixtures/valid/` and `tests/fixtures/invalid/` directories
- [ ] Create `tests/fixtures/README.md` with a placeholder for UE version and fixture generation notes
- [ ] Add `binary` attribute for `.uasset` / `.umap` / `.bin` in `.gitattributes`
- [ ] Confirm `cargo check --workspace` passes

## Related

- Next: Issue #2 — `crates/shared`: AssetType + FPackageVersion
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 1
