# アーキテクチャ・設計

## 全体アーキテクチャ

```text
uasset-lens/
 ├─ crates/
 │   ├─ scanner               # .uasset スキャン・メタデータ抽出
 │   ├─ asset-db              # SQLite による Asset インデックス管理
 │   ├─ dependency-graph      # Hard/Soft Reference 解析・グラフ構築
 │   ├─ impact-analyzer       # 削除・リネーム影響分析
 │   ├─ redirector-analyzer   # Redirector 検出・分析
 │   ├─ dead-asset-detector   # 未使用 Asset 検出
 │   ├─ duplicate-detector    # 重複 Asset 検出
 │   ├─ bp-analyzer           # Blueprint 静的解析
 │   ├─ level-analyzer        # Level / World Partition 分析
 │   ├─ material-analyzer     # Material 複雑度分析
 │   ├─ lint-engine           # Linter ルールエンジン
 │   ├─ budget-tracker        # Performance Budget 管理
 │   ├─ git-diff              # Blueprint / Asset 差分解析
 │   ├─ watcher               # ファイルシステム監視（Watch Mode）
 │   ├─ reporter              # HTML / Markdown レポート生成
 │   ├─ dashboard             # egui GUI ダッシュボード
 │   ├─ cli                   # CLI コマンド定義（clap）
 │   └─ shared                # 共通型・ユーティリティ
 │
 ├─ apps/
 │   ├─ uasset-lens-cli
 │   └─ uasset-lens-desktop
 │
 └─ docs/
```

---

## Phase 1 ワークスペース構成

### クレート一覧（Phase 1）

| クレート | 種別 | 役割 |
|---|---|---|
| `crates/shared` | lib | 共通型定義・エラー型（`AssetPath`・`AssetType`・`FPackageVersion`） |
| `crates/scanner` | lib | `.uasset` バイナリパーサー・メタデータ抽出 |
| `crates/asset-db` | lib | SQLite による Asset インデックス管理・差分スキャン |
| `crates/dependency-graph` | lib | 依存グラフ構築・循環依存検出 |
| `crates/dead-asset-detector` | lib | 未使用 Asset・孤立 Asset 検出 |
| `crates/impact-analyzer` | lib | 削除・リネーム影響範囲分析（Phase 1 は `dependency-graph.find_impact()` の薄いラッパー。Phase 2 でリネーム安全性検査・Soft Reference 解析を追加） |
| `crates/redirector-analyzer` | lib | Redirector 検出・分析 |
| `crates/cli` | lib | clap コマンド定義・ハンドラロジック・出力フォーマット・ディレクトリウォーク・設定ファイル読み込み |
| `apps/uasset-lens-cli` | bin | エントリポイント（`main.rs` のみ） |

### クレート間依存関係

```text
shared
  ├── scanner
  ├── asset-db
  ├── dependency-graph
  │     ├── dead-asset-detector
  │     └── impact-analyzer
  └── redirector-analyzer

cli ← shared
    ← scanner
    ← asset-db
    ← dependency-graph
    ← dead-asset-detector
    ← impact-analyzer
    ← redirector-analyzer

apps/uasset-lens-cli ← cli（main.rs のみ）
```

依存の原則:
- `shared` はどのクレートにも依存しない（依存グラフの底）
- `scanner`・`asset-db`・`redirector-analyzer` は `shared` にのみ依存する
- `dependency-graph` は `shared` と `petgraph` にのみ依存する（DB/IO 非依存の純粋グラフ計算層）
- `dead-asset-detector`・`impact-analyzer` は `dependency-graph` に依存する
- `cli` は全ライブラリクレートに依存する。ディレクトリウォーク（`walkdir`）・設定ファイル読み込み（`toml`）も `cli` が担う
- `apps/uasset-lens-cli` は `cli` にのみ依存する（薄いエントリポイント）

### `cli` / `apps` 分離方針

| パッケージ | 種別 | 内容 |
|---|---|---|
| `crates/cli` | ライブラリクレート | clap コマンド定義・ハンドラロジック・出力フォーマット |
| `apps/uasset-lens-cli` | バイナリクレート | `main.rs` のみ（数行）。`crates/cli` を呼び出す |

将来の GUI（`apps/uasset-lens-desktop`）からも `crates/cli` のロジックを再利用できる設計とする。

### `shared` crate の内容

型定義・エラー型のみ。ユーティリティ関数は各クレートに持たせる。

| ファイル | 内容 |
|---|---|
| `asset_path.rs` | `AssetPath` 型 |
| `asset_type.rs` | `AssetType` enum |
| `error.rs` | 共通エラー型（`thiserror` ベース） |
| `version.rs` | `FPackageVersion` |

### ワークスペース設定方針

- 依存クレートのバージョンは `[workspace.dependencies]` で一元管理する
- `edition = "2021"` を全クレートで統一する
- `resolver = "2"` を使用する（Rust 2021 デフォルト）

---

## 内部データモデル

### Graph Model

```text
Asset -> Asset
Blueprint -> Component
Material -> Texture
Blueprint -> Blueprint
```

### Database 化する理由

以下を可能にするため。

- 高速検索
- Trend Analysis
- Historical Diff
- Query System
- Large Scale Analysis

### クエリ例

```sql
SELECT *
FROM blueprint_dependencies
WHERE circular = true;
```
