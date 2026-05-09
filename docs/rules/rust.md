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

### tokio は Phase 1 では使用しない

tokio は Watch Mode (Phase 4) 以降に追加する。Phase 1 のライブラリクレート（`scanner`・`asset-db` 等）に
`tokio` を追加することを禁止する。

---

## Cross-platform Path Handling

ファイルシステムパスには必ず `std::path::PathBuf` / `Path` を使用する。
OS 固有のパス区切り文字（`\` / `/`）をリテラルで書いてはならない。

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

`AssetPath`（`/Game/...` 形式のゲームパス）は常に `/` 区切りの `String` であり、
ファイルシステムパスとは別物として明確に区別する。

---

## Database (SQLite)

### バッチ書き込みはトランザクションで囲む

SQLite は 1 行ごとにトランザクションを作ると著しく遅くなる。
`scan` コマンドのように大量の upsert を行う場合は単一トランザクションに包む。

```rust
// ❌ 1 行ずつコミット（遅い）
for meta in &results {
    db.upsert_asset(meta)?;
}

// ✅ トランザクション一括
let tx = conn.transaction()?;
for meta in &results {
    upsert_asset_tx(&tx, meta)?;
}
tx.commit()?;
```

### rusqlite は bundled feature を使う

SQLite はシステム依存を避けるため静的リンク（`features = ["bundled"]`）する。

---

## Logging

`tracing` を使う。production コードで `println!` / `eprintln!` を使用してはならない。
stdout/stderr への書き込みは `cli` クレートのみ行う（`docs/rules/cli-output.md` 参照）。

```rust
// ✅ Structured logging with field names
tracing::info!(file_count = %n, "Scan started");
tracing::debug!(path = %path.display(), "Parsing file");
tracing::warn!(path = %path.display(), reason = %e, "Skipping file");
tracing::error!(error = ?e, "Fatal error in scan handler");
```

Log levels:
- `error`: 正確性に影響する予期しない失敗
- `warn`: 回復可能な問題（ファイルスキップ、パースエラー）
- `info`: ライフサイクルイベント（スキャン開始/終了、DB オープン）
- `debug`: ファイル単位のトレース（パス、タイミング）

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
// ✅ AssetPath は String だが、ラップすることで生 String との誤用を防ぐ
pub struct AssetPath(String);
```

### Builder pattern for complex construction

3 つ以上のオプションフィールドを持つ構造体にはビルダーパターンを使う。
必須フィールドは `new()` に置く。

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

クロージャや `Arc` のクローンには、なぜクローンが必要かをコメントで明記する。

```rust
// clone required: rayon::spawn requires 'static + Send
let content_root = content_root.clone();
```

### No `unsafe` without justification

`unsafe` ブロックには必ず `// SAFETY:` コメントで不変条件を説明する。

```rust
// SAFETY: `ptr` is valid for the lifetime of this function and aligned to T.
let value = unsafe { ptr.read() };
```

### No dead code in committed branches

コミット前に未使用の `use`・関数・変数を削除する。
`#[allow(dead_code)]` を使う場合は残す理由をコメントで記述する。

---

## コメントポリシー

- コメントは「なぜ」のみ書く（「何をするか」は書かない）。
- 自明なコードにコメントは不要。
- **バイナリ形式・外部仕様（UE5 バイナリレイアウト等）を文書化する関数・構造体**では
  multi-line コメントを許容する。1 行に収まらない仕様はこちらを優先する。

```rust
// ✅ OK — バイナリ仕様の文書化（multi-line 許容）
// FObjectImport layout (UE5.4, FileVersionUE5 >= 1012):
//   ClassPackage (FName: i32 index + i32 number) = 8 bytes
//   ClassName    (FName: i32 index + i32 number) = 8 bytes
//   OuterIndex   (i32)                           = 4 bytes
//   ObjectName   (FName: i32 index + i32 number) = 8 bytes
//   PackageName  (FName: i32 index + i32 number) = 8 bytes  [UE5 addition]
//   bImportOptional (serialised as i32)          = 4 bytes
pub fn parse_import_table(...) { ... }

// ✅ OK — 非自明な WHY を 1 行で
// from_utf8 consumes the Vec directly — no copy in the happy path
String::from_utf8(bytes).unwrap_or_else(...)

// ❌ NG — 自明なコードに "what" コメント
// Loop over all entries
for entry in &entries { ... }
```
