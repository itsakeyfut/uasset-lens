# uasset-lens — Rust Coding Standards

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rayon Documentation](https://docs.rs/rayon)

---

## Error Handling

### Crate-level policy

| Layer | Tool | Usage |
|-------|------|-------|
| Library crates (`scanner`, `asset-db`, `dependency-graph`, `dead-asset-detector`, `impact-analyzer`, `redirector-analyzer`) | `thiserror` | Typed, match-able error enums |
| Application layer (`cli` crate, `apps/uasset-lens-cli`) | `anyhow` | Contextual error propagation |

```rust
// ✅ scanner/src/error.rs — typed error for library crate
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("invalid magic number: {0:#x}")]
    InvalidMagic(u32),
    #[error("unsupported file version: legacy={0}, ue5={1}")]
    UnsupportedVersion(i32, u32),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ✅ cli/src/commands/scan.rs — anyhow for application layer
pub fn handle_scan(project_dir: &Path, opts: &ScanOpts) -> anyhow::Result<()> {
    let db = AssetDb::open(&db_path)
        .context("Failed to open uasset-lens.db")?;
    Ok(())
}
```

### `unwrap()` / `expect()` policy

`unwrap()` and `expect()` are **only permitted inside `#[cfg(test)]` blocks.**

```rust
// ❌ FORBIDDEN in production code
let record = db.get_asset(&path).unwrap();

// ✅ Use ? propagation
let record = db.get_asset(&path)
    .context("Failed to query asset")?;

// ✅ In tests, unwrap is acceptable
#[test]
fn detect_should_return_dead_asset() {
    let graph = build_test_graph();
    assert!(!detect(&graph).is_empty());
}
```

---

## Parallelism

### rayon for CPU-bound work

Use `rayon` for parallel file scanning. Never manage threads manually.

```rust
// ✅ Parallel file parsing
use rayon::prelude::*;

let results: Vec<Result<AssetMetadata, _>> = files
    .par_iter()
    .map(|path| parse_file(path, content_root))
    .collect();
```

`scan_files()` produces an immutable `Vec<AssetMetadata>`. CLI writes to DB sequentially after collection.
Do not share mutable state across rayon threads.

### tokio is not used in Phase 1

tokio is added in Phase 5 (Watch Mode) and later. Do not add `tokio` to Phase 1 library crates
(`scanner`, `asset-db`, etc.).

---

## Cross-platform Path Handling

Always use `std::path::PathBuf` / `Path` for filesystem paths.
Never write OS-specific path separators (`\` / `/`) as literals.

```rust
// ❌ FORBIDDEN — hardcoded separator
let content = project_dir.to_string() + "\\Content";

// ✅ Use PathBuf methods
let content = project_dir.join("Content");

// ❌ FORBIDDEN — string-based path comparison
if path.to_string_lossy().starts_with("Content\\") { ... }

// ✅ Use Path methods
if path.starts_with("Content") { ... }
```

`AssetPath` (a `/Game/...`-style game path) is always a `/`-delimited `String`.
Keep it clearly separate from filesystem paths.

---

## Database (SQLite)

### Wrap batch writes in a transaction

SQLite is significantly slower when each row commits its own transaction.
Wrap bulk upserts (such as in the `scan` command) in a single transaction.

```rust
// ❌ One commit per row (slow)
for meta in &results {
    db.upsert_asset(meta)?;
}

// ✅ Single transaction for the batch
let tx = conn.transaction()?;
for meta in &results {
    upsert_asset_tx(&tx, meta)?;
}
tx.commit()?;
```

### Use the bundled feature for rusqlite

Statically link SQLite (`features = ["bundled"]`) to avoid system dependency issues.

---

## Logging

Use `tracing`. Never use `println!` / `eprintln!` in production code.
Only the `cli` crate writes to stdout/stderr (see `docs/rules/cli-output.md`).

```rust
// ✅ Structured logging with field names
tracing::info!(file_count = %n, "Scan started");
tracing::debug!(path = %path.display(), "Parsing file");
tracing::warn!(path = %path.display(), reason = %e, "Skipping file");
tracing::error!(error = ?e, "Fatal error in scan handler");
```

Log levels:
- `error`: unexpected failures that affect correctness
- `warn`: recoverable problems (file skipped, parse error)
- `info`: lifecycle events (scan start/end, DB open)
- `debug`: per-file trace (path, timing)

---

## Type Design

### Prefer named structs over tuples

```rust
// ❌ Opaque tuple
fn parse_result(&self) -> (AssetPath, Vec<AssetPath>) { ... }

// ✅ Named struct
pub struct AssetMetadata {
    pub asset_path:   AssetPath,
    pub dependencies: Vec<AssetPath>,
    // ...
}
```

### Newtype for domain values

```rust
// ✅ AssetPath wraps String to prevent misuse with raw strings
pub struct AssetPath(String);
```

### Builder pattern for complex construction

Use the builder pattern for structs with three or more optional fields.
Place required fields in `new()`.

---

## Code Quality

### Iterators over manual loops

```rust
// ❌ Manual accumulation
let mut dead = Vec::new();
for node in graph.nodes() {
    if graph.in_degree(&node.path) == 0 {
        dead.push(node.path.clone());
    }
}

// ✅ Iterator pipeline
let dead: Vec<_> = graph.nodes()
    .filter(|node| graph.in_degree(&node.path) == 0)
    .map(|node| node.path.clone())
    .collect();
```

### Non-obvious clones must be annotated

Document why a clone is necessary for closures and `Arc` clones.

```rust
// clone required: rayon::spawn requires 'static + Send
let content_root = content_root.clone();
```

### No `unsafe` without justification

Every `unsafe` block must have a `// SAFETY:` comment explaining the invariant it relies on.

```rust
// SAFETY: `ptr` is valid for the lifetime of this function and aligned to T.
let value = unsafe { ptr.read() };
```

### No dead code in committed branches

Remove unused `use` statements, functions, and variables before committing.
If `#[allow(dead_code)]` is necessary, explain the reason with a comment.

---

## Comment Policy

- Comments explain **why** only — never what the code does.
- Self-evident code needs no comment.
- **Functions and structs that document binary formats or external specs** (e.g., UE5 binary layout)
  may use multi-line block comments when a single line cannot capture the spec.

```rust
// ✅ OK — binary spec documentation (multi-line permitted)
// FObjectImport layout (UE5.4, FileVersionUE5 >= 1012):
//   ClassPackage (FName: i32 index + i32 number) = 8 bytes
//   ClassName    (FName: i32 index + i32 number) = 8 bytes
//   OuterIndex   (i32)                           = 4 bytes
//   ObjectName   (FName: i32 index + i32 number) = 8 bytes
//   PackageName  (FName: i32 index + i32 number) = 8 bytes  [UE5 addition]
//   bImportOptional (serialised as i32)          = 4 bytes
pub fn parse_import_table(...) { ... }

// ✅ OK — non-obvious WHY in one line
// from_utf8 consumes the Vec directly — no copy in the happy path
String::from_utf8(bytes).unwrap_or_else(...)

// ❌ NG — "what" comment on self-evident code
// Loop over all entries
for entry in &entries { ... }
```
