# Phase 6 — 可視化・レポート

## ゴール

CLI 解析結果を egui GUI ダッシュボードと HTML/Markdown レポートとして可視化する。
Level/Map 固有の分析機能を追加し、プロジェクト全体の健全性を一目で把握できる状態にする。

## 前提条件

Phase 5 完了（Watch Mode・CI 統合が動作）

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/level-analyzer` | lib | 新規作成 |
| `crates/report-generator` | lib | 新規作成 |
| `apps/uasset-lens-desktop` | bin | 新規作成（egui GUI） |
| `crates/cli` | lib | 拡張（`report` コマンド） |

## スコープ外

- Blueprint グラフのインタラクティブ編集（閲覧のみ）
- UE エディタ連携プラグイン

---

## 実装タスク

### 1. `crates/level-analyzer` 実装

- [ ] Level（`.umap`）内 Asset の集計
  - World に含まれる Actor タイプ別カウント
  - Level 間の依存関係（`/Game/Maps/` 以下を起点に DependencyGraph を走査）
- [ ] `LevelMetrics` 構造体定義
  - `actor_count: u32`
  - `referenced_assets: Vec<AssetPath>`
  - `dependency_levels: Vec<AssetPath>`（他 Level への参照）
- [ ] World Partition 検知（Export テーブルの `WorldPartition` クラスの存在有無）
- [ ] 単体テスト（World フィクスチャを使用）

---

### 2. `crates/report-generator` 実装

- [ ] `ReportConfig` 構造体定義
  - `format: ReportFormat`（Html / Markdown）
  - `output_path: PathBuf`
  - `include_sections: Vec<ReportSection>`
- [ ] `ReportSection` enum
  - Summary / DeadAssets / Cycles / BlueprintMetrics / Budget / Duplicates / Levels
- [ ] HTML レポート生成
  - テンプレート文字列（外部ファイル不要・コード埋め込み）
  - CSS インライン（外部 CDN 依存なし・オフライン動作）
  - セクション別テーブル・折りたたみ対応
- [ ] Markdown レポート生成
  - GitHub Flavored Markdown 準拠
  - 各セクションの見出し・テーブル・コードブロック
- [ ] 単体テスト（空データ・フルデータ両方で出力が正常生成されること）

---

### 3. `crates/cli` — `report` コマンド追加

- [ ] `report <project_dir>` コマンドハンドラ
  - `--format html` / `--format markdown`（デフォルト: markdown）
  - `-o <path>` / `--output <path>`（デフォルト: `uasset-lens-report.md`）
  - `--sections <section,...>`（省略時は全セクション）
- [ ] 出力先ファイルが既存の場合の上書き確認（`-y` フラグで自動 yes）
- [ ] exit codes: 正常 → `0`、実行エラー → `2`

---

### 4. `apps/uasset-lens-desktop` — egui GUI 実装

#### 4-1. アプリ骨格

- [ ] `eframe` + `egui` によるウィンドウ作成
- [ ] プロジェクトディレクトリ選択（ファイルダイアログ）
- [ ] 選択後に `scanner` → `asset-db` → `dependency-graph` を順次実行
- [ ] スキャン進捗表示（プログレスバー）

#### 4-2. ダッシュボード画面

- [ ] **サマリーパネル**
  - 総 Asset 数 / 総サイズ / 未参照 Asset 数 / サイクル数
- [ ] **未参照 Asset 一覧**
  - 型 / パス / サイズでソート可能なテーブル
  - 選択 Asset のパスをクリップボードにコピー
- [ ] **循環依存一覧**
  - サイクルを構成する Asset パスのリスト表示
- [ ] **Blueprint 複雑度ランキング**
  - ノード数 / EventTick / Cast 数でソート可能
- [ ] **Asset 検索**
  - 型・名前・サイズでリアルタイムフィルタ

#### 4-3. 依存グラフビュー（簡易）

- [ ] 選択 Asset の直接依存 / 被依存を平面リストで表示
  - `impact` コマンドの出力相当
  - インタラクティブなグラフ描画は将来対応（Phase 6 スコープ外）

---

## 完了条件

### 機能要件

- [ ] `uasset-lens report ./Project --format html -o report.html` が動作する
- [ ] `uasset-lens report ./Project --format markdown` が GitHub で正常レンダリングされる
- [ ] `apps/uasset-lens-desktop` がプロジェクトを開いてスキャン結果を表示できる
- [ ] GUI ダッシュボードの未参照 Asset 一覧・サイクル一覧・Blueprint ランキングが表示される
- [ ] GUI の Asset 検索がリアルタイムフィルタリングで動作する
- [ ] `level-analyzer` が `.umap` ファイルから `LevelMetrics` を抽出できる

### テスト要件

- [ ] `cargo test --workspace` がパスする
- [ ] `report-generator` の HTML / Markdown 出力が非空で生成されることをテスト
- [ ] `level-analyzer` の World フィクスチャテストがパスする

### 品質要件

- [ ] `cargo clippy --workspace -- -D warnings` が警告ゼロ
- [ ] `cargo fmt --check` がパスする
- [ ] GUI はウィンドウサイズ変更に対してレイアウトが崩れない
- [ ] HTML レポートが外部 CDN / ネットワーク接続なしで表示できる

### リリース要件

- [ ] `apps/uasset-lens-desktop` が Windows / macOS / Linux でビルド成功
- [ ] GitHub Releases に CLI バイナリ + GUI バイナリの両方が添付される
- [ ] `README.md` に GUI インストール手順・スクリーンショットが追加される
