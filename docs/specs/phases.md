# Phase 設計

6フェーズ構成。**Phase 2 完了時点が MVP**。

| Phase | テーマ | MVP |
|-------|--------|-----|
| 1 | Foundation: Binary Scanner | — |
| 2 | Core Analysis | **MVP** |
| 3 | CLI 完成 | — |
| 4 | 静的解析 | — |
| 5 | 開発フロー統合 | — |
| 6 | 可視化・レポート | — |

詳細は `docs/roadmap/phase{N}/ROADMAP.md` を参照。

---

## Phase 1 — Foundation: Binary Scanner

### 目標

`.uasset` / `.umap` バイナリをパースし、Asset メタデータと依存関係を SQLite に索引できる状態にする。
`uasset-lens scan ./Project` が動作すれば完了。

### 実装範囲

#### 1. Asset Scanner

- `.uasset` / `.umap` バイナリパース（Magic / NameTable / ImportTable / ExportTable）
- AssetType 自動判定
- メタデータ抽出（path / type / size / last_modified / dependencies）
- rayon 並列スキャン + mtime 差分スキャン

#### 2. Asset DB

- SQLite による Asset インデックス管理
- mtime 差分スキャン対応スキーマ
- 削除検知・バッチ upsert

#### 3. CLI: `scan` コマンド

```bash
uasset-lens scan ./Project
uasset-lens scan ./Project --full-scan
```

---

## Phase 2 — Core Analysis ✅ MVP

### 目標

「この Asset を削除して大丈夫か？」に CLI 単体で即答できる状態にする。
`graph` / `dead-assets` / `impact` の 3 コマンドが動作した時点で **MVP 達成**。

### 実装範囲

#### 1. Dependency Graph

- Hard Reference 解析
- Circular Dependency 検出（Tarjan SCC）
- Impact 分析（direct / transitive 分離）

#### 2. Dead Asset Detector

- 未参照 Asset 検出（in_degree == 0）

#### 3. CLI: 3 コマンド追加

```bash
uasset-lens graph ./Project
uasset-lens graph ./Project --cycles-only
uasset-lens dead-assets ./Project
uasset-lens dead-assets ./Project --type Texture2D
uasset-lens impact /Game/Characters/BP_Player
```

---

## Phase 3 — CLI 完成

### 目標

残りの全 CLI コマンドと設定ファイルを実装し、OSS として公開できるレベルに仕上げる。

### 実装範囲

#### 1. Redirector Analyzer

- ObjectRedirector Asset の検出・列挙

#### 2. Asset Search CLI

- 型 / サイズ / パス / 未参照フラグによるフィルタ検索
- glob パターン対応

#### 3. 設定ファイル（`.uasset-lens.toml`）

```toml
[scan]
exclude_paths = ["Content/Dev/", "Content/Test/"]
```

#### 4. CLI: 2 コマンド追加

```bash
uasset-lens redirectors ./Project
uasset-lens find ./Project --type Texture2D --larger-than 4194304
uasset-lens find ./Project --unreferenced --type StaticMesh
uasset-lens find ./Project --path "**/Characters/**"
```

---

## Phase 4 — 静的解析

### 目標

「削除の安全性」から「Blueprint / Asset 品質分析」へ価値を拡大する。
`lint` コマンドを CI 品質ゲートとして使える状態にする。

### 実装範囲

#### 1. Blueprint Analyzer

- Node Count / Branch Count / Event Tick / Cast / Dependency Depth
- 複雑度閾値判定（Linter から呼び出し）

#### 2. Duplicate Asset Detector

- 同名 Asset の重複検出
- Texture 重複検出（サイズ + 型 + 名前ベース）

#### 3. Linter

- 命名規則（T_ / M_ / SM_ / BP_ 等プレフィックス）
- Texture サイズ上限
- Blueprint 複雑度
- exit code `1` で CI ゲートとして機能

#### 4. Material Analyzer

- テクスチャサンプル数
- MaterialInstance チェーン深度

#### 5. Performance Budget Tracking

```toml
[budget]
Texture2D.max_size = 4194304    # 4 MB
SoundWave.max_size = 2097152    # 2 MB
```

#### 6. CLI: 4 コマンド追加

```bash
uasset-lens blueprint ./Project
uasset-lens lint ./Project
uasset-lens budget ./Project
uasset-lens duplicates ./Project
```

---

## Phase 5 — 開発フロー統合

### 目標

ツールを開発ワークフローに組み込む。
Asset 変更をリアルタイム検知する Watch Mode、Blueprint 構造の Git 差分可視化、GitHub Actions CI 統合を実現する。

### 実装範囲

#### 1. Watch Mode

- `notify` クレートによるファイルシステム監視
- デバウンス処理 + 変更時即時再スキャン + 問題通知

#### 2. Git Diff Analyzer

- `git show HEAD:path` で旧バージョンを取得して比較
- 依存関係の追加 / 削除、Blueprint メトリクスの変化を表示

#### 3. CI Integration

- GitHub Actions サンプルワークフロー
- `lint` の exit code `1` によるパイプライン停止

#### 4. CLI: `watch` コマンド追加

```bash
uasset-lens watch ./Project
```

---

## Phase 6 — 可視化・レポート

### 目標

CLI 解析結果を egui GUI ダッシュボードと HTML/Markdown レポートで可視化する。
Level / Map 固有の分析機能を追加する。

### 実装範囲

#### 1. Level / Map Analyzer

- Level 内 Actor タイプ別カウント
- Level 間依存グラフ
- World Partition 検知

#### 2. Report Generator

- HTML レポート（オフライン動作、CDN 不要）
- Markdown レポート（GitHub Flavored Markdown）

#### 3. GUI Dashboard（egui / eframe）

- スキャン結果のダッシュボード表示
- 未参照 Asset / 循環依存 / Blueprint ランキング
- リアルタイム Asset 検索

#### 4. CLI: `report` コマンド追加 + GUI バイナリ

```bash
uasset-lens report ./Project --format html -o report.html
uasset-lens report ./Project --format markdown
```
