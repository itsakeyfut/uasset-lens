# Phase 4 — 静的解析

## ゴール

「削除の安全性」から「Blueprint / Asset 品質分析」へ価値を拡大する。
Linter コマンドを CI 品質ゲートとして使える状態にする。

## 前提条件

Phase 3 完了（全 CLI コマンドが動作）

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/scanner` | lib | 拡張（Export プロパティ解析 — Phase 2 パーサー） |
| `crates/bp-analyzer` | lib | 新規作成 |
| `crates/duplicate-detector` | lib | 新規作成 |
| `crates/lint-engine` | lib | 新規作成 |
| `crates/material-analyzer` | lib | 新規作成 |
| `crates/budget-tracker` | lib | 新規作成 |
| `crates/asset-db` | lib | 拡張（Blueprint メトリクス保存用スキーマ追加） |
| `crates/cli` | lib | 拡張（新コマンド群） |

## スコープ外

- Watch Mode / Git Diff（Phase 5）
- GUI / Report（Phase 6）

---

## 技術的前提：パーサー Phase 2 の実装

Phase 1 のパーサーは Header + NameTable + ImportTable + ExportTable ヘッダーのみ解析した。
Phase 4 の Blueprint 解析には **Export オブジェクトのプロパティデータ** の読み取りが必要。

これは Phase 1 パーサーとは別の技術的挑戦であり、Phase 4 の冒頭で先行実装する。

### パーサー追加実装の範囲

- [x] `parser/properties.rs` — FProperty / FTag の解析（Blueprint のノード情報取得）
- [x] Blueprint の Export オブジェクトから以下を抽出できるようにする
  - ノード総数（グラフノード数）
  - EventTick の使用有無
  - Cast ノードの数
  - 外部 Blueprint への参照数（Dependency Depth に相当）
- [x] `AssetMetadata` に Blueprint 専用フィールドを追加、または専用構造体を用意

---

## 実装タスク

### 1. `crates/bp-analyzer` 実装

- [x] `BlueprintMetrics` 構造体定義
  - `node_count: u32`
  - `event_tick_count: u32`
  - `cast_count: u32`
  - `dependency_depth: u32`
- [x] `analyze(metadata: &AssetMetadata) -> Option<BlueprintMetrics>` 実装
  - Blueprint / AnimBlueprint / UserWidget のみ対象
  - 他の型は `None` を返す
- [x] 複雑度閾値の判定ロジック
  - `is_complex(metrics, thresholds) -> Vec<Warning>` — Linter から呼ばれる
- [x] 単体テスト（Blueprint フィクスチャを使用）

---

### 2. `crates/duplicate-detector` 実装

- [x] 同名 Asset の重複検出
  - 異なるパスに同じファイル名を持つ Asset を列挙
- [x] Texture 重複検出（同一ファイルサイズ + 型 + 名前ベースの近似判定）
  - ファイルハッシュによる完全一致は将来対応（`xxhash-rust`）
- [x] `DuplicateGroup` 構造体定義（重複 Asset のグループ）
- [x] 単体テスト

---

### 3. `crates/lint-engine` 実装

#### ルール定義

- [x] `LintRule` trait の設計
  - `check(asset: &AssetRecord, metrics: Option<&BlueprintMetrics>) -> Vec<LintViolation>`
- [x] `LintViolation` 構造体（severity / rule_id / message / asset_path）
- [x] Phase 4 で実装するルール
  - **命名規則**: プレフィックス検証（T_ / M_ / SM_ / BP_ / SK_ 等）
  - **Texture サイズ**: ファイルサイズ上限
  - **Blueprint 複雑度**: ノード数・EventTick 使用数
  - **LOD なし検知**: 将来の型情報取得後に対応（未実装、スコープ外）

#### 設定ファイル拡張

- [x] `.uasset-lens.toml` に `[lint]` セクションを追加（Phase 3 の `[scan]` に追記）
  ```toml
  [lint]
  naming_prefix.Blueprint = "BP_"
  naming_prefix.Texture2D = "T_"
  blueprint_max_nodes = 200
  blueprint_max_event_tick = 1
  ```

---

### 4. `crates/material-analyzer` 実装

- [x] テクスチャサンプル数の計測（scanner の `material_texture_samples` フィールドから取得）
- [x] MaterialInstance チェーン深度の計算（依存グラフから算出）
- [x] `MaterialMetrics` 構造体定義
- [x] 単体テスト

---

### 5. `crates/budget-tracker` 実装

- [x] `.uasset-lens.toml` の `[budget]` セクション定義
  ```toml
  [budget]
  Texture2D.max_size = 4194304    # 4 MB
  SoundWave.max_size = 2097152    # 2 MB
  ```
- [x] `check_budget(assets, config) -> BudgetReport` 実装
  - カテゴリ別の超過 Asset 一覧
  - 超過数 / 超過率サマリー

---

### 6. `crates/cli` — 新コマンド追加

- [x] `blueprint <project_dir>` コマンド
  - 複雑度ランキング・警告一覧を表示
- [x] `lint <project_dir>` コマンド
  - 全ルールを実行、違反一覧を表示
  - exit code `1` で CI ゲートとして使用可能
- [x] `budget <project_dir>` コマンド
  - 予算超過 Asset 一覧・サマリー表示
- [x] `duplicates <project_dir>` コマンド
  - 重複 Asset グループの表示

---

## 完了条件

### 機能要件

- [x] `uasset-lens blueprint ./Project` が Blueprint の複雑度メトリクスを表示する
- [x] `uasset-lens lint ./Project` が命名規則・複雑度違反を報告する
- [x] `uasset-lens lint ./Project` が違反ありで exit code `1` を返す（CI ゲートとして機能）
- [x] `.uasset-lens.toml` の `[lint]` 設定が反映される
- [x] `uasset-lens budget ./Project` が予算超過 Asset を報告する
- [x] `uasset-lens duplicates ./Project` が重複 Asset グループを検出する
- [x] 全新コマンドで `--format json` が動作する

### テスト要件

- [x] `cargo test --workspace` がパスする
- [x] Blueprint フィクスチャで `bp-analyzer` のメトリクス抽出が検証されている
- [x] `lint-engine` の各ルールが個別にテストされている
- [x] `budget-tracker` の超過判定ロジックがテストされている

### 品質要件

- [x] `cargo clippy --workspace -- -D warnings` が警告ゼロ
- [x] `cargo fmt --check` がパスする
- [x] `docs/specs/phases.md` Phase 4 の全機能が実装されている
