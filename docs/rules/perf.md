# uasset-lens — Performance Rules

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rayon Documentation](https://docs.rs/rayon)
- [Criterion Benchmarking](https://bheisler.github.io/criterion.rs/book/)

---

## Performance Targets

**release ビルド** (`cargo build --release`) での目標値。

| メトリクス | 目標値 | 計測方法 |
|-----------|--------|---------|
| フルスキャン — 1,000 assets | **< 5 秒** | `time uasset-lens scan --full-scan ./Project` |
| フルスキャン — 10,000 assets | **< 30 秒** | 同上 |
| メモリ使用量 — 100,000 assets | **< 100 MB** | OS プロセスモニター |
| `impact` コマンド応答 | **< 1 秒** | スキャン済み状態・100k assets のグラフ |
| `dead-assets` コマンド応答 | **< 1 秒** | 同上 |
| `graph --cycles-only` 応答 | **< 2 秒** | 同上 |

各リリース前に手動で検証する。

---

## Binary Parsing Performance

バイナリパーサーのパターンは `docs/rules/binary-parser.md` を参照。

### ファイルを 1 回だけ読む

ファイル全体を `Vec<u8>` に読み込んでからスライスをパースする。
シーク操作を繰り返さない。

```rust
// ✅ Single read, parse from slice
let data = std::fs::read(path)?;
let metadata = parse_asset(&data, content_root)?;
```

### パースパスでのアロケーションを最小化

- Name Table のエントリは `&str` スライスで参照し、長期保持が必要な場合のみ `.to_owned()`
- Import Table のフィルタリング（`/Script/` / `/Engine/` 除外）は `AssetPath` 変換より前に行う

```rust
// ✅ Filter early — avoid allocating discarded entries
let deps: Vec<AssetPath> = raw_imports.iter()
    .filter(|s| s.starts_with("/Game/"))
    .map(|s| AssetPath::new(s).expect("already validated"))
    .collect();
```

---

## Parallel Scanning

### rayon でファイルレベルの並列化

`.uasset` ファイルは独立してパースできる。`par_iter()` を使う。
rayon ワーカースレッド間で可変状態を共有しない。

```rust
// ✅ Parallel parse
let results: Vec<_> = files
    .par_iter()
    .map(|path| parse_file(path, content_root))
    .collect();
```

### DB への書き込みは並列化しない

rayon で並列パースした結果を収集してから、SQLite へは逐次書き込みを行う。
DB 書き込みを rayon ワーカーから呼ぶことを禁止する。

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

### 全 Asset を一度にメモリに展開しない

CLI コマンドは必要なデータのみ DB から取得する。
100k assets 分の `AssetMetadata` を `Vec` で保持しない。
`graph`/`impact`/`dead-assets` は DB のエッジリストのみからグラフを構築する。

### サイズが既知ならあらかじめ確保する

```rust
// ✅ Avoid repeated reallocations
let mut deps = Vec::with_capacity(import_count as usize);
```

### バッファは drop せず clear して再利用する

```rust
// ✅ Reuse allocation
self.import_buf.clear(); // clears contents, keeps capacity
```

---

## Benchmarking with Criterion

クリティカルパスには [Criterion](https://bheisler.github.io/criterion.rs/book/) でベンチマークを作成する。
ベンチマークは各クレートの `benches/` に置く。

### ベンチマーク対象

| クレート | 対象 | ファイル |
|---------|------|---------|
| `scanner` | `scan_files()` — ファイル数を変えて計測 | `crates/scanner/benches/scan.rs` |
| `scanner` | `parse_file()` — 1 ファイルのパース時間 | `crates/scanner/benches/parse.rs` |
| `dependency-graph` | `DependencyGraph::build()` | `crates/dependency-graph/benches/build.rs` |
| `dependency-graph` | `find_impact()`・`find_cycles()` | `crates/dependency-graph/benches/queries.rs` |
| `asset-db` | `filter_changed()`・`all_edges()` | `crates/asset-db/benches/queries.rs` |

### ベンチマーク構造

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

### 実行方法

```bash
cargo bench -p scanner
cargo bench -p dependency-graph
```

ベンチマークは CI では実行しない。パフォーマンスに影響する変更前後に手動で実行する。

```bash
# 変更前のベースラインを保存
cargo bench -p scanner -- --save-baseline before
# 変更後に比較
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

メモリ計測: Windows は Task Manager / Process Hacker、macOS は Instruments、Linux は `heaptrack`。
