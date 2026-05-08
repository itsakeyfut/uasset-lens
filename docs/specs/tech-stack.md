# 技術スタック

## コア

- Rust

## 対象 Unreal Engine バージョン

- UE5 のみ（5.1 以降）

## 対象ファイル形式

- **解析対象**: 開発中のコンテンツ（`.uasset` / `.umap`）のみ
- **永続的スコープ外**: IoStore 形式（`.utoc` / `.ucas`）— Cook / Package 済みビルドは対象外。将来フェーズでも対応しない。ただしパーサーのアーキテクチャは拡張可能に設計する。

## GUI

- egui

## Database

- SQLite（将来的に DuckDB へ移行可能な設計にする）

## .uasset パーサー実装方針

- **`.uasset` 解析ロジックは完全自前実装**（既存の `.uasset` / UE 解析クレートは使用しない）
- バイナリ解析の汎用ユーティリティ（`nom`・`byteorder`）は積極的に使用する
- Binary Parsing スキルのポートフォリオ価値を最大化するため
- Asset Registry（`AssetRegistry.bin`）は補助的に参照する（存在する場合のみ）

### 解析対象フォーマット（UE5）

`.uasset` は以下の構造を持つバイナリファイル。

```
FPackageFileSummary（ファイルヘッダー）
 ├─ Magic Number      : 0x9E2A83C1
 ├─ LegacyFileVersion / FileVersionUE5
 ├─ Name Table        : パッケージ内で使われる全文字列
 ├─ Import Table      : 他 Asset への Hard Reference（FObjectImport）
 ├─ Export Table      : このパッケージが定義するオブジェクト（FObjectExport）
 └─ Soft Reference    : プロパティデータに埋め込まれた Soft Object Path
```

### Phase 別の実装深度

- Phase 1: ヘッダー + Name Table + Import/Export Table（依存解析に必要な最小限）
- Phase 2: Export データのプロパティ解析（Blueprint ノードの読み取り）
- Phase 3 以降: Soft Reference の完全解析

## グラフ処理

- petgraph

## CLI

- clap

## スキャン方式

- **デフォルト**: 差分スキャン（mtime ベース）
  - DB に各ファイルの最終更新時刻（mtime）を保存
  - 前回スキャン時から mtime が変化したファイルのみ再解析
- **`--full-scan` オプション**: 全 Asset を強制再解析

DB への保存項目（scanner）:
- `file_path`
- `last_modified`（mtime）

## CLI 出力フォーマット

- テキスト（デフォルト、人間向け）
- JSON（`--format json` オプション、CI / 他ツール連携向け）

## Serialization

- serde

## 並列処理

| 用途 | ライブラリ |
|---|---|
| CPU バウンド（スキャン・解析） | `rayon` |
| 非同期 I/O・イベント駆動（Watch Mode・将来の HTTP 連携） | `tokio` |

## エラーハンドリング

- アプリ層（CLI / GUI）: `anyhow`
- ライブラリ層（各 crate）: `thiserror`

## ログ・トレース

- `tracing`（構造化ログ・スパン情報）

## その他ライブラリ

| 用途 | ライブラリ |
|---|---|
| ディレクトリ再帰ウォーク（`cli` crate で使用） | `walkdir` |
| ファイル監視（Watch Mode） | `notify` |
| TOML パース（設定ファイル） | `toml` |
| HTML テンプレート（Report Generator） | `askama` |
| ファイルハッシュ（将来の重複検出用） | `xxhash-rust` |
