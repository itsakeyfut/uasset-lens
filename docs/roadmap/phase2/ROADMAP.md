# Phase 2 — Core Analysis ✅ MVP

## ゴール

「この Asset を削除して大丈夫か？」という問いに CLI 単体で即答できる状態にする。
`graph`・`dead-assets`・`impact` の 3 コマンドが動作した時点で **MVP 達成**。

## 前提条件

Phase 1 完了（`scan` コマンドが動作し、lens.db に Asset が索引された状態）

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/dependency-graph` | lib | 新規作成 |
| `crates/dead-asset-detector` | lib | 新規作成 |
| `crates/impact-analyzer` | lib | 新規作成（Phase 1 stub） |
| `crates/cli` | lib | 拡張（3 コマンド追加） |

## スコープ外

- `redirectors` コマンド・`find` コマンド・設定ファイル（Phase 3）
- Blueprint / Linter 解析（Phase 4 以降）
- `impact-analyzer` の本格実装（Phase 3 以降で拡張）

---

## 実装タスク

### 1. `crates/dependency-graph` 実装

#### 1-1. 型定義

- [ ] `AssetNode` 構造体
  - `path: AssetPath`・`asset_type: AssetType`
- [ ] `ImpactResult` 構造体
  - `direct: Vec<AssetPath>`（1 ホップで直接参照）
  - `transitive: Vec<AssetPath>`（2 ホップ以上。direct を含まない）
- [ ] `DependencyGraph` 構造体
  - `graph: DiGraph<AssetNode, ()>`
  - `index: HashMap<AssetPath, NodeIndex>`

#### 1-2. 公開 API 実装

- [ ] `build(nodes, edges) -> Self`
  - 全 Asset ノードを先に登録してからエッジを張る
  - エッジの一方または両方が nodes になくても処理を継続する（未登録 to_path は孤立ノードとして追加）
- [ ] `nodes() -> impl Iterator<Item = &AssetNode>`
- [ ] `in_degree(path: &AssetPath) -> usize`
  - 入次数（何個の Asset から参照されているか）
- [ ] `find_cycles() -> Vec<Vec<AssetPath>>`
  - `petgraph::algo::tarjan_scc` を使用
  - 要素数 2 以上の SCC のみ返す（自己参照は除外）
  - 各サイクルは出発点に戻るパスとして返す（例: `[A, B, C, A]`）
- [ ] `find_impact(target: &AssetPath) -> ImpactResult`
  - `Reversed` グラフ上で BFS
  - 1 ホップのみ → `direct`
  - 2 ホップ以上 → `transitive`（target 自身は含まない）

#### 1-3. 単体テスト

- [ ] `build()` — 孤立ノードあり・エッジあり・両方のテスト
- [ ] `find_cycles()` — サイクルあり / DAG / 2 ノード相互参照
- [ ] `find_impact()` — 直接のみ / 推移的あり / 影響なし / 到達不能ノード
- [ ] `in_degree()` — 0 / 正の値 / 複数参照元

---

### 2. `crates/dead-asset-detector` 実装

- [ ] `detect(graph: &DependencyGraph) -> Vec<AssetPath>` 実装
  - `graph.nodes()` を走査し `in_degree == 0` のノードを収集
- [ ] 単体テスト（孤立ノードあり / なし / 混在）

---

### 3. `crates/impact-analyzer` 実装（Phase 1 stub）

- [ ] `pub use dependency_graph::ImpactResult;` の re-export
- [ ] Phase 1 では cli が `dependency_graph.find_impact()` を直接呼ぶ設計のため、このクレートは Phase 2 以降の拡張プレースホルダーとして空実装
- [ ] `Cargo.toml` に `dependency-graph` への依存を追加

---

### 4. `crates/cli` — 3 コマンド追加

#### 4-1. 共通: DependencyGraph のロードユーティリティ

- [ ] `load_graph(db: &AssetDb) -> Result<DependencyGraph>`
  - `db.all_assets()` → `Vec<AssetNode>` に変換
  - `db.all_edges()` → `Vec<(AssetPath, AssetPath)>` に変換
  - `DependencyGraph::build()` を呼ぶ
  - 全コマンドで再利用するヘルパーとして cli クレート内に定義

#### 4-2. `graph` コマンド

- [ ] ハンドラ実装
  - DB が存在しない場合は「Run scan first」エラー（exit 2）
  - `load_graph()` → `find_cycles()` → テキスト出力
  - `--cycles-only` フラグ: サイクルのみ表示
- [ ] テキスト出力実装（`docs/specs/cli-design.md` の `graph` 仕様に準拠）
- [ ] JSON 出力実装（`total_assets / total_edges / cycles`）
- [ ] exit codes: `--cycles-only` でサイクルあり → `1`、クリーン → `0`

#### 4-3. `dead-assets` コマンド

- [ ] ハンドラ実装
  - `load_graph()` → `dead_asset_detector::detect()` → テキスト出力
  - `--type <AssetType>` フラグ: 型フィルタ
  - ゼロ件でも出力する
- [ ] テキスト出力実装（件数 + パス一覧 + ファイルサイズ + AssetType）
- [ ] JSON 出力実装（AssetRecord 配列）
- [ ] exit codes: 未参照 Asset あり → `1`、なし → `0`

#### 4-4. `impact` コマンド

- [ ] ハンドラ実装
  - `<asset_path>` 引数: ゲームパス（`/Game/...`）またはファイルシステムパスを受け付ける
  - ファイルシステムパス渡しの場合 → `AssetPath::from_fs_path()` で変換
  - `load_graph()` → `find_impact()` → テキスト出力
  - 対象 Asset が DB に存在しない場合はエラー（exit 2）
- [ ] テキスト出力実装（Direct / Transitive / Total）
- [ ] JSON 出力実装（`target / direct / transitive / total`）
- [ ] exit codes: impact あり → `1`、なし → `0`

---

## 完了条件

### 機能要件

- [ ] `uasset-lens graph ./Project` が依存グラフの概要と循環依存を正しく表示する
- [ ] `uasset-lens graph --cycles-only ./Project` が循環依存のみ表示する
- [ ] `uasset-lens dead-assets ./Project` が未参照 Asset の一覧を表示する
- [ ] `uasset-lens dead-assets --type Texture2D ./Project` が型フィルタで絞り込める
- [ ] `uasset-lens impact /Game/Characters/BP_Player` が direct / transitive 影響範囲を表示する
- [ ] `uasset-lens impact ./Project/Content/Characters/BP_Player.uasset`（ファイルシステムパス）が動作する
- [ ] 全コマンドで `--format json` が仕様の JSON スキーマに準拠した出力を返す
- [ ] exit codes が全コマンドで正しく動作する

### テスト要件

- [ ] `cargo test --workspace` が全プラットフォームでパスする
- [ ] `dependency-graph` の全単体テストがパスする
  - サイクル検出（複数サイクル・巨大グラフ含む）
  - impact（direct/transitive 分離の正確性）
  - in_degree / nodes
- [ ] `dead-asset-detector` の全単体テストがパスする
- [ ] `graph`・`dead-assets`・`impact` コマンドの exit code テストがパスする
- [ ] `--format json` 出力が各コマンドで正しいキーを持つことを検証している

### 品質要件

- [ ] `cargo clippy --workspace -- -D warnings` が警告ゼロでパスする
- [ ] `cargo fmt --check` がパスする

### パフォーマンス要件（リリースビルド）

- [ ] `graph`・`dead-assets`・`impact` の各コマンドが 1,000 assets のグラフで 1 秒以内に応答する

### MVP デモ要件

- [ ] **実際の UE5 プロジェクトで 3 コマンドをデモ実行し、正しい結果が返ることを確認する**
  - 既知の未参照 Asset が `dead-assets` で検出される
  - 既知の依存 Asset が `impact` に正しく表示される
  - 循環依存がある場合は `graph --cycles-only` で検出される
