# 開発戦略・ロードマップ

## MVP

### 最初に作るべきもの

#### MVP 機能（Phase 2 完了時点）

- uasset scan
- dependency graph
- circular dependency detection
- dead asset detection
- impact analysis（削除・リネーム影響範囲の表示）

#### MVP の目的

以下を成立させる。

> 「この Asset を削除して大丈夫か？」

---

## 6フェーズ構成

| Phase | テーマ | 主要コマンド | MVP |
|-------|--------|-------------|-----|
| 1 | Foundation: Binary Scanner | `scan` | — |
| 2 | Core Analysis | `graph` / `dead-assets` / `impact` | **MVP** |
| 3 | CLI 完成 | `redirectors` / `find` + 設定ファイル | — |
| 4 | 静的解析 | `blueprint` / `lint` / `budget` / `duplicates` | — |
| 5 | 開発フロー統合 | `watch` + CI ドキュメント | — |
| 6 | 可視化・レポート | `report` + GUI ダッシュボード | — |

各フェーズの詳細タスク・完了条件は `docs/roadmap/phase{N}/ROADMAP.md` を参照。

---

## 開発戦略

最初から全機能を実装しない。Phase 2 完了でリリースし、フィードバックを得ながら段階的に拡張する。

### 実装順序（フェーズ内優先順位）

#### Phase 1
1. `shared` crate（共通型定義）
2. `scanner` crate（バイナリパーサー）
3. `asset-db` crate（SQLite）
4. `cli` crate（`scan` コマンド）

#### Phase 2
1. `dependency-graph` crate
2. `dead-asset-detector` crate
3. `impact-analyzer` crate（stub）
4. `cli` 拡張（3 コマンド）

#### Phase 3
1. `redirector-analyzer` crate
2. `asset-db` glob 対応
3. `cli` 拡張（2 コマンド + 設定ファイル）
4. README / `cargo publish` 準備

#### Phase 4
1. パーサー Phase 2（Export プロパティ解析）
2. `bp-analyzer` crate
3. `duplicate-detector` crate
4. `lint-engine` crate
5. `material-analyzer` / `budget-tracker` crates
6. `cli` 拡張（4 コマンド）

#### Phase 5
1. `watcher` crate
2. `git-diff` crate
3. `cli` 拡張（`watch` コマンド）
4. CI 統合ドキュメント

#### Phase 6
1. `level-analyzer` crate
2. `report-generator` crate
3. `cli` 拡張（`report` コマンド）
4. `apps/uasset-lens-desktop`（egui GUI）

---

## 将来的な拡張案

### GitHub PR Integration

PR 時に以下を自動通知:

- BP Complexity 増加
- Circular Dependency 検出
- Asset Budget 超過

### Plugin System

将来的には Analyzer / Rule を Plugin 化し、プロジェクト固有ルールを追加できる設計にする。

---

## プロジェクト名

**uasset-lens**

- CLI バイナリ名: `uasset-lens`
- GUI バイナリ名: `uasset-lens-desktop`
- Cargo パッケージ名: `uasset-lens`
- リポジトリ名: `uasset-lens`
