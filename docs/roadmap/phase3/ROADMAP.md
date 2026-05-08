# Phase 3 — CLI 完成

## ゴール

残りの全 CLI コマンドと設定ファイルを実装し、ツールを公開できるレベルに仕上げる。
このフェーズ完了後、当初の Phase 1 として想定していた全機能が揃い、README と合わせて OSS として公開可能になる。

## 前提条件

Phase 2 完了（MVP の 3 コマンドが動作する状態）

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/redirector-analyzer` | lib | 新規作成 |
| `crates/asset-db` | lib | 拡張（find_assets の glob 対応） |
| `crates/cli` | lib | 拡張（`redirectors`・`find` コマンド、設定ファイル対応） |

## スコープ外

- Blueprint / Linter 解析（Phase 4 以降）
- Watch Mode / Git Diff（Phase 5）
- GUI / Report（Phase 6）

---

## 実装タスク

### 1. `crates/redirector-analyzer` 実装

- [ ] `detect(graph: &DependencyGraph) -> Vec<AssetPath>` 実装
  - `graph.nodes()` を走査し `asset_type == AssetType::ObjectRedirector` を収集
- [ ] 単体テスト（ObjectRedirector あり / なし / 混在型）

---

### 2. `.uasset-lens.toml` 設定ファイル対応

- [ ] `ConfigFile` 構造体定義（`cli` クレート内、または専用モジュール）
  ```toml
  [scan]
  exclude_paths = ["Content/Dev/", "Content/Test/"]
  ```
- [ ] プロジェクトルートから `.uasset-lens.toml` を自動検索するロジック
  - `<project_dir>/.uasset-lens.toml` が存在すれば読み込む
  - 存在しない場合はデフォルト設定で動作（エラーにしない）
- [ ] `scan` コマンドの `exclude_paths` 適用
  - walkdir のディレクトリ列挙時に前方一致で除外
  - content_root からの相対パスで比較

---

### 3. `crates/cli` — `redirectors` コマンド

- [ ] ハンドラ実装
  - `load_graph()` → `redirector_analyzer::detect()` → テキスト出力
  - 件数 + パス一覧を表示
- [ ] テキスト出力実装（`docs/specs/cli-design.md` の `redirectors` 仕様に準拠）
- [ ] JSON 出力実装（ObjectRedirector パス配列）
- [ ] フェーズ 1 スコープ注記をテキスト出力末尾に表示
  - `Note: redirect target resolution is available in Phase 4 analysis.`
- [ ] exit codes: Redirector あり → `1`、なし → `0`

---

### 4. `crates/asset-db` — `find_assets()` glob 対応

- [ ] `AssetFilter.path_pattern` の glob マッチ実装
  - `--path "**/Characters/**"` のようなパターンに対応
  - `glob` または `globset` クレートを使用
  - SQL の LIKE ではなく Rust 側でフィルタリング（シンプルさを優先）

---

### 5. `crates/cli` — `find` コマンド

- [ ] ハンドラ実装
  - `AssetFilter` を CLI オプションから構築
  - `db.find_assets(&filter)` を呼び出す
  - `--unreferenced` フラグ: グラフを構築して dead_asset_detector::detect() と交差
- [ ] CLI オプション実装
  - `--type <AssetType>`（例: `Texture2D`・`Blueprint`）
  - `--larger-than <bytes>` / `--smaller-than <bytes>`（ファイルサイズフィルタ）
  - `--unreferenced`（未参照 Asset のみ）
  - `--path <pattern>`（glob パターン）
- [ ] テキスト出力実装（パス + 型 + サイズ一覧）
- [ ] JSON 出力実装（`docs/specs/cli-design.md` の `find` JSON スキーマに準拠）
- [ ] ゼロ件でも正常終了（exit 0）

---

### 6. README 作成

- [ ] プロジェクト概要（コンセプト・解決する課題）
- [ ] インストール方法（`cargo install` / GitHub Releases）
- [ ] クイックスタート（scan → impact の最短手順）
- [ ] 全コマンドのリファレンス（オプション・出力例）
- [ ] `.uasset-lens.toml` 設定例
- [ ] システム要件（対応 UE バージョン・OS）

---

## 完了条件

### 機能要件

- [ ] `uasset-lens redirectors ./Project` が ObjectRedirector Asset を列挙する
- [ ] `uasset-lens find ./Project --type Texture2D --larger-than 4194304` が動作する
- [ ] `uasset-lens find ./Project --unreferenced --type StaticMesh` が動作する
- [ ] `uasset-lens find ./Project --path "**/Characters/**"` が glob パターンで絞り込める
- [ ] `.uasset-lens.toml` の `exclude_paths` が `scan` 時に正しく適用される
- [ ] 設定ファイルが存在しない場合はデフォルト設定で正常動作する
- [ ] 全 6 コマンドで `--format json` が仕様スキーマに準拠した出力を返す

### テスト要件

- [ ] `cargo test --workspace` が全プラットフォームでパスする
- [ ] `redirector-analyzer` の全単体テストがパスする
- [ ] `find` コマンドの各フィルタオプションが個別・複合でテストされている
- [ ] `exclude_paths` が正しくスキャン除外される統合テストがある
- [ ] `.uasset-lens.toml` の parse / デフォルトフォールバックがテストされている

### 品質要件

- [ ] `cargo clippy --workspace -- -D warnings` が警告ゼロでパスする
- [ ] `cargo fmt --check` がパスする
- [ ] README が英語で記述されている（OSS として公開することを想定）

### 公開準備要件

- [ ] ライセンスファイル（MIT または Apache-2.0）が配置されている
- [ ] `cargo publish` のドライランが通る（`cargo publish --dry-run`）
- [ ] GitHub Releases 用のバイナリビルドが Windows / macOS / Linux で成功する
