# uasset-lens — Testing Standards

## References

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [proptest Book](https://proptest-rs.github.io/proptest/intro.html)
- [Criterion Book](https://bheisler.github.io/criterion.rs/book/)

---

## Philosophy

Test **behavior**, not implementation. A test that breaks only when observable behavior changes
is a good test. A test that breaks when you rename an internal field is not.

---

## Test Naming Convention

All test functions follow the pattern:

```
<feature>_should_<expected_result>
```

```rust
// ✅ Good names — describe what the system should do
fn parse_blueprint_should_extract_import_dependencies()
fn asset_path_should_reject_empty_string()
fn detect_should_return_assets_with_no_incoming_edges()
fn find_impact_should_separate_direct_and_transitive()
fn filter_changed_should_exclude_files_with_unchanged_mtime()

// ❌ Bad names — describe implementation, not behavior
fn test_parse()
fn test_graph()
```

---

## Test Layers

### 1. Unit tests (primary)

Place unit tests in a `#[cfg(test)] mod tests { ... }` block inside the source file.
Each test exercises a single function or method in isolation.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_path_should_reject_path_with_extension() {
        assert!(AssetPath::new("/Game/Chars/BP_Player.uasset").is_err());
    }
}
```

### 2. Integration tests

Integration tests live in `crates/<crate>/tests/` and use **real fixture files** from `tests/fixtures/`.
Fixture 構成・ソースは `docs/specs/testing.md` を参照。

```rust
// crates/scanner/tests/integration.rs
#[test]
fn scan_files_should_parse_blueprint_fixture() {
    let root = Path::new("tests/fixtures/valid");
    let result = scanner::scan_files(&[root.join("BP_Simple.uasset")], root);
    let meta = &result.assets[0];
    assert_eq!(meta.asset_type, AssetType::Blueprint);
}
```

### 3. Property-based tests (proptest)

任意の入力に対して不変条件が成立することを検証する。uasset-lens での対象：

- `AssetPath::new()` — 任意の文字列で panic しない
- `DependencyGraph::find_impact()` — 任意のグラフで panic しない
- `find_cycles()` — 返されるサイクルが自己一貫している

```toml
[dev-dependencies]
proptest = "1"
```

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn asset_path_new_should_never_panic(s in ".*") {
        let _ = AssetPath::new(s); // must not panic regardless of input
    }
}
```

### 4. Criterion benchmarks

`docs/rules/perf.md` を参照。パフォーマンスクリティカルなパスにのみ追加する。

---

## What to Test per Crate

### `shared`

| 対象 | テスト |
|------|-------|
| `AssetPath::new()` | 空文字・先頭スラッシュなし・拡張子付きを拒否; 有効パスを受け入れる |
| `AssetPath::from_fs_path()` | content_root 外のファイルを拒否; `/Game/` プレフィックスへの正しい変換 |
| `AssetPath::package_name()` | オブジェクト名サフィックスを正しく除去 |
| `AssetType` serde | 全バリアントが serialize/deserialize でロスなく往復する |

### `scanner`

| 対象 | テスト |
|------|-------|
| `scan_files()` — valid fixtures | 正しい `AssetType` 検出; dependency 一覧に期待パスが含まれる; skipped なし |
| `scan_files()` — bad magic | `ScanError::InvalidMagic` で skipped に入る |
| `scan_files()` — truncated | `ScanError::UnexpectedEof` で skipped に入る |
| `scan_files()` — `.umap` | `AssetType::World` が返る |
| `scan_files()` — ObjectRedirector | 型が正しく検出される |
| Import フィルタリング | `/Script/` / `/Engine/` 参照が dependencies に含まれない |
| 並列正確性 | rayon 並列の結果がシングルスレッドと一致する |

### `asset-db`

| 対象 | テスト |
|------|-------|
| `open()` | 初回でスキーマ作成; 既存 DB は正常オープン |
| `upsert_asset()` + `get_asset()` | 保存・取得の往復 |
| `filter_changed()` | 新規ファイルを返す; mtime 変化なしを除外; mtime 変化ありを返す |
| `all_edges()` | upsert 後に正しいエッジペアが返る |
| `all_assets()` | upsert した全 Asset が返る |
| `find_assets()` with `AssetFilter` | type・size・path パターンが単独/複合で機能する |
| `delete_asset()` | レコード削除; 依存エッジが CASCADE で削除される |

### `dependency-graph`

| 対象 | テスト |
|------|-------|
| `build()` | エッジなし孤立ノードが存在する; エッジが正しく追加される |
| `find_cycles()` | サイクルあり→返す; DAG→空; 2 ノード相互参照→検出 |
| `find_impact()` — 直接のみ | `direct` 正しい; `transitive` 空 |
| `find_impact()` — 推移的あり | direct と transitive が正しく分離 |
| `find_impact()` — 影響なし | 両リスト空 |
| `in_degree()` | 孤立ノードは 0; 参照されるノードは正しいカウント |
| `nodes()` | 孤立ノードを含む全ノードが返る |

### `dead-asset-detector`

| 対象 | テスト |
|------|-------|
| `detect()` — 孤立ノードあり | 結果に含まれる |
| `detect()` — 全て参照済み | 空リスト |
| `detect()` — 混在 | 未参照のみ返す |

### `redirector-analyzer`

| 対象 | テスト |
|------|-------|
| `detect()` — ObjectRedirector あり | 結果に含まれる |
| `detect()` — Redirector なし | 空リスト |
| `detect()` — 混在型 | ObjectRedirector のパスのみ返す |

### `cli`（integration）

| 対象 | テスト |
|------|-------|
| `scan` exit codes | クリーン → `0`; ディレクトリ不在 → `2` |
| `graph --cycles-only` | サイクルあり → `1`; クリーン → `0` |
| `dead-assets` | 未参照 Asset あり → `1` |
| `--format json` | 全コマンドが仕様に沿ったパース可能な JSON を出力する |
| `impact` JSON | `direct`/`transitive`/`total` キーが存在する |

---

## What NOT to Test

- 内部フィールド名・プライベート構造体のレイアウト
- サードパーティクレートの内部（`rusqlite`・`petgraph`）
- プラットフォーム固有のファイルシステム挙動（抽象パスでテストする）
- マクロが生成したコード
- ロジックを含まない trivial な getter/setter

---

## Test Helpers

共有のテストユーティリティは `#[cfg(test)] mod tests` 内のヘルパー関数、
または統合テスト用の `tests/common/` に定義する。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph_a_refs_b() -> DependencyGraph {
        DependencyGraph::build(
            vec![
                AssetNode { path: AssetPath::new("/Game/A").unwrap(), asset_type: AssetType::Blueprint },
                AssetNode { path: AssetPath::new("/Game/B").unwrap(), asset_type: AssetType::Blueprint },
            ],
            vec![(
                AssetPath::new("/Game/A").unwrap(),
                AssetPath::new("/Game/B").unwrap(),
            )],
        )
    }

    #[test]
    fn find_impact_should_return_direct_referencing_asset() {
        let graph = make_graph_a_refs_b();
        let result = graph.find_impact(&AssetPath::new("/Game/B").unwrap());
        assert_eq!(result.direct, vec![AssetPath::new("/Game/A").unwrap()]);
        assert!(result.transitive.is_empty());
    }
}
```
