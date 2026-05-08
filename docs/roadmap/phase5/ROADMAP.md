# Phase 5 — 開発フロー統合

## ゴール

ツールを開発ワークフローに組み込む。Asset 変更をリアルタイム検知する Watch Mode、
Blueprint 構造の Git 差分可視化、GitHub Actions CI パイプラインへの統合を実現する。

## 前提条件

Phase 4 完了（静的解析コマンド群が動作）

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/watcher` | lib | 新規作成 |
| `crates/git-diff` | lib | 新規作成 |
| `crates/cli` | lib | 拡張（`watch` コマンド） |

## スコープ外

- GUI / Report（Phase 6）

---

## 実装タスク

### 1. `crates/watcher` 実装

- [ ] `notify` クレートを使用したファイルシステム監視
- [ ] `.uasset` / `.umap` の変更・作成・削除イベントを検知
- [ ] デバウンス処理（短時間の連続変更を 1 イベントにまとめる）
- [ ] 変更ファイルを `scanner::scan_files()` に渡して即時再スキャン
- [ ] 再スキャン後に新たな問題（新しい dead asset・新しいサイクル等）を通知
- [ ] 単体テスト（モックファイルシステムイベントで動作確認）

---

### 2. `crates/git-diff` 実装

- [ ] `git show HEAD:path/to/asset.uasset` で以前のバージョンのバイナリを取得
- [ ] 旧バージョン・新バージョン双方を `scanner` でパースして比較
- [ ] `AssetDiff` 構造体定義
  - 依存関係の追加 / 削除
  - AssetType の変更
  - Blueprint メトリクスの変化（node_count・event_tick 等）
- [ ] 差分出力フォーマット定義

---

### 3. `crates/cli` — `watch` コマンド追加

- [ ] `watch <project_dir>` コマンドハンドラ
  - 起動時に初回スキャンを実行
  - ファイル変更を監視してインクリメンタル更新
  - 変更のたびに問題一覧を再表示
  - `Ctrl+C` で終了

---

### 4. CI 統合ドキュメント

- [ ] GitHub Actions ワークフローの設定例を作成
  ```yaml
  - name: Check circular dependencies
    run: uasset-lens graph --cycles-only ./Project
  - name: Lint assets
    run: uasset-lens lint ./Project
  ```
- [ ] CI での `.uasset` ファイルの扱いに関するガイド（Git LFS vs 直接コミット）
- [ ] `docs/` または `README.md` に CI Integration セクションを追加

---

## 完了条件

### 機能要件

- [ ] `uasset-lens watch ./Project` が起動し、ファイル変更を検知して再分析する
- [ ] Watch Mode で新たな問題が発生すると即時に通知される
- [ ] `Ctrl+C` で Watch Mode が正常終了する
- [ ] GitHub Actions サンプルワークフローが実際のリポジトリで動作する
- [ ] `uasset-lens lint` が CI で exit code `1` を返してパイプラインを止められる

### テスト要件

- [ ] `cargo test --workspace` がパスする
- [ ] Watch Mode のデバウンス処理がテストされている

### 品質要件

- [ ] `cargo clippy --workspace -- -D warnings` が警告ゼロ
- [ ] CI 統合ドキュメントが実際に動作する設定例を含んでいる
