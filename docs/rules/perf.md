# uasset-lens — Performance Rules

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rayon Documentation](https://docs.rs/rayon)
- [Criterion Benchmarking](https://bheisler.github.io/criterion.rs/book/)

---

## Performance Targets

Targets measured on a **release build** (`cargo build --release`).

| Metric | Target | How to measure |
|--------|--------|---------------|
| Full scan — 1,000 assets | **< 5 s** | `time uasset-lens scan --full-scan ./Project` |
| Full scan — 10,000 assets | **< 30 s** | same |
| Memory — 100,000 assets | **< 100 MB** | OS process monitor |
| `impact` command response | **< 1 s** | pre-scanned state, 100k-asset graph |
| `dead-assets` command response | **< 1 s** | same |
| `graph --cycles-only` response | **< 2 s** | same |

Verify manually before each release.

---

## Binary Parsing Performance

See `docs/rules/binary-parser.md` for binary parser patterns.

### Read each file exactly once

Read the entire file into a `Vec<u8>`, then parse from slices.
Do not repeat seek operations.

```rust
// ✅ Single read, parse from slice
let data = std::fs::read(path)?;
let metadata = parse_asset(&data, content_root)?;
```

### Minimize allocations on the parse path

- Reference Name Table entries as `&str` slices; call `.to_owned()` only when the value must outlive the parse
- Filter Import Table entries (`/Script/` / `/Engine/` exclusion) before converting to `AssetPath`

```rust
// ✅ Filter early — avoid allocating discarded entries
let deps: Vec<AssetPath> = raw_imports.iter()
    .filter(|s| s.starts_with("/Game/"))
    .map(|s| AssetPath::new(s).expect("already validated"))
    .collect();
```

---

## Parallel Scanning

### File-level parallelism with rayon

`.uasset` files can be parsed independently. Use `par_iter()`.
Do not share mutable state across rayon worker threads.

```rust
// ✅ Parallel parse
let results: Vec<_> = files
    .par_iter()
    .map(|path| parse_file(path, content_root))
    .collect();
```

### Do not parallelize DB writes

Collect rayon parallel parse results first, then write to SQLite sequentially.
DB writes must never be called from a rayon worker.

```rust
// ✅ Parallel parse → sequential DB write
let parsed: Vec<_> = files.par_iter()
    .filter_map(|p| parse_file(p, content_root).ok())
    .collect();

let tx = conn.transaction()?;
for meta in &parsed {
    upsert_asset_tx(&tx, meta)?;
}
tx.commit()?;
```

---

## Memory Management

### Never expand all assets into memory at once

CLI commands fetch only the data they need from the DB.
Do not hold 100k `AssetMetadata` entries in a `Vec`.
`graph` / `impact` / `dead-assets` build the in-memory graph from the DB edge list only.

### Pre-allocate when the size is known

```rust
// ✅ Avoid repeated reallocations
let mut deps = Vec::with_capacity(import_count as usize);
```

### Reuse buffers by calling clear() instead of dropping them

```rust
// ✅ Reuse allocation
self.import_buf.clear(); // clears contents, keeps capacity
```

---

## Benchmarking with Criterion

Add [Criterion](https://bheisler.github.io/criterion.rs/book/) benchmarks to critical paths.
Place benchmarks in each crate's `benches/` directory.

### What to benchmark

| Crate | Target | File |
|-------|--------|------|
| `scanner` | `scan_files()` — vary file count | `crates/scanner/benches/scan.rs` |
| `scanner` | `parse_file()` — single-file parse time | `crates/scanner/benches/parse.rs` |
| `dependency-graph` | `DependencyGraph::build()` | `crates/dependency-graph/benches/build.rs` |
| `dependency-graph` | `find_impact()` / `find_cycles()` | `crates/dependency-graph/benches/queries.rs` |
| `asset-db` | `filter_changed()` / `all_edges()` | `crates/asset-db/benches/queries.rs` |

### Benchmark structure

```rust
// crates/scanner/benches/parse.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::Path;

fn bench_parse_blueprint(c: &mut Criterion) {
    let path = Path::new("tests/fixtures/valid/BP_Simple.uasset");
    let root = Path::new("tests/fixtures/valid");

    c.bench_function("parse_blueprint", |b| {
        b.iter(|| {
            scanner::scan_files(
                black_box(&[path.to_path_buf()]),
                black_box(root),
            )
        })
    });
}

criterion_group!(benches, bench_parse_blueprint);
criterion_main!(benches);
```

### How to run

```bash
cargo bench -p scanner
cargo bench -p dependency-graph
```

Do not run benchmarks in CI. Run them manually before and after changes that affect performance.

```bash
# Save baseline before the change
cargo bench -p scanner -- --save-baseline before
# Compare after the change
cargo bench -p scanner -- --load-baseline before --save-baseline after
```

---

## Profiling

```powershell
# Windows
Measure-Command { .\target\release\uasset-lens.exe scan --full-scan ./Project }
```

```bash
# macOS / Linux
time ./target/release/uasset-lens scan --full-scan ./Project
```

Memory measurement: Task Manager / Process Hacker on Windows, Instruments on macOS, `heaptrack` on Linux.
