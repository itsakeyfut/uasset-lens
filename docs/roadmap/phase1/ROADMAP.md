# Phase 1 — Foundation: Binary Scanner

## ゴール

`.uasset` / `.umap` バイナリファイルをパースし、Asset のメタデータと依存関係を SQLite に索引できる状態にする。
このフェーズ完了後、`uasset-lens scan ./Project` が動作する。他の全コマンドはこのフェーズが土台になる。

## 対象クレート

| クレート | 種別 | 作成 / 拡張 |
|---------|------|------------|
| `crates/shared` | lib | 新規作成 |
| `crates/scanner` | lib | 新規作成 |
| `crates/asset-db` | lib | 新規作成 |
| `crates/cli` | lib | 新規作成（`scan` コマンドのみ） |
| `apps/uasset-lens-cli` | bin | 新規作成（main.rs のみ） |

## スコープ外

- `dependency-graph` / 各 analyzer（Phase 2）
- `graph` / `dead-assets` / `impact` 等のコマンド（Phase 2）
- `find` コマンド・設定ファイル（Phase 3）
- Blueprint / Linter 解析（Phase 4 以降）

---

## 実装タスク

### 1. Cargo ワークスペース初期設定

- [ ] ルート `Cargo.toml` 作成（workspace 宣言、`resolver = "2"`、`edition = "2021"`）
- [ ] `[workspace.dependencies]` に全依存バージョンを一元定義
  - `thiserror`・`serde`・`nom`・`byteorder`・`rayon`・`walkdir`
  - `rusqlite`（`features = ["bundled"]`）・`clap`・`anyhow`・`tracing`・`petgraph`
  - `serde_json`・`toml`
- [ ] `.gitignore` に `target/`・`.uasset-lens/` を追加
- [ ] `tests/fixtures/` ディレクトリを用意し `.gitattributes` で binary 属性を設定

---

### 2. `crates/shared` 実装

- [ ] `asset_path.rs` — `AssetPath(String)` newtype
  - `new(s)` → バリデーション（空文字・先頭 `/` なし・拡張子付きを Err）
  - `as_str()`・`package_name()`（`.BP_Player` サフィックス除去）
  - `from_fs_path(content_root, file_path)` → `/Game/...` パスへ変換
  - `AssetPathError` enum（Empty / MissingLeadingSlash / InvalidCharacter / NotUnderContentRoot）
- [ ] `asset_type.rs` — `AssetType` enum
  - Blueprint / BlueprintInterface / AnimBlueprint / UserWidget
  - StaticMesh / SkeletalMesh / Texture2D
  - Material / MaterialInstance / MaterialFunction
  - SoundWave / SoundCue / AnimSequence / AnimMontage
  - DataTable / DataAsset / World / ObjectRedirector / Unknown(String)
  - `#[derive(serde::Serialize, serde::Deserialize)]` を付与
- [ ] `version.rs` — `FPackageVersion`
  - `legacy_version: i32`・`file_version_ue4: u32`・`file_version_ue5: u32`
  - `is_ue5()` → `legacy_version == -8 && file_version_ue5 > 0`
- [ ] `lib.rs` — 全型を pub re-export
- [ ] `shared` 単体テスト（`AssetPath` バリデーション全ケース・`from_fs_path`・`package_name`）

---

### 3. `crates/scanner` 実装

#### 3-1. エラー型

- [ ] `error.rs` — `ScanError` enum
  - `InvalidMagic(u32)`・`UnsupportedVersion(i32, u32)`・`UnexpectedEof`・`Io(#[from] std::io::Error)`

#### 3-2. パーサー実装

- [ ] `parser/header.rs` — `FPackageFileSummary` パース
  - Magic Number 検証（`0x9E2A83C1`）
  - `LegacyFileVersion`・`FileVersionUE5` 読み取り → `FPackageVersion`
  - NameTable / ImportTable / ExportTable の **オフセット** と **カウント** を取得
  - UE5 でない場合（`legacy_version != -8`）→ `ScanError::UnsupportedVersion`
- [ ] `parser/name_table.rs` — Name Table パース
  - オフセットから N 個の FString を読み込み `Vec<String>` として返す
  - FString パース（長さプレフィックス + UTF-8 + null 終端）
  - UTF-16LE（負の長さ）は Phase 1 未対応 → スキップして空文字返し（警告ログ）
- [ ] `parser/import.rs` — Import Table パース（Hard Reference）
  - `FObjectImport` から `ClassPackage`・`ObjectName` を解決して完全パスを組み立てる
  - `/Script/`・`/Engine/` プレフィックスを除外（変換前にフィルタ）
  - `/Game/` で始まるもののみ `AssetPath` に変換して返す
- [ ] `parser/export.rs` — Export Table パース（AssetType 判定）
  - 先頭 `FObjectExport` の ClassIndex から NameTable を引いてクラス名を取得
  - クラス名 → `AssetType` へのマッピング（`Unknown(String)` でフォールバック）
  - `.umap` は問答無用で `AssetType::World`

#### 3-3. スキャナー本体

- [ ] `AssetMetadata` 構造体定義
  - `asset_path: AssetPath`・`file_path: PathBuf`・`asset_type: AssetType`
  - `file_size: u64`・`last_modified: u64`・`dependencies: Vec<AssetPath>`
- [ ] `scan_files(files: &[PathBuf], content_root: &Path) -> ScanResult` 実装
  - `rayon::par_iter()` で並列パース
  - パースエラーは `ScanResult.skipped` に入れてスキャン継続
  - `tracing::warn!` でスキップログを出力
- [ ] `ScanResult`・`SkippedFile` 型定義

#### 3-4. テストフィクスチャ整備と単体テスト

- [ ] `tests/fixtures/valid/` に最小構成の実 `.uasset` ファイルを配置
  - `BP_Simple.uasset`（Blueprint・Import あり）
  - `T_Rock.uasset`（Texture2D）
  - `SM_Cube.uasset`（StaticMesh）
  - `M_Basic.uasset`（Material）
  - `Redirect.uasset`（ObjectRedirector）
  - `L_TestMap.umap`（World）
- [ ] `tests/fixtures/invalid/` に合成エラーケースを配置
  - `bad_magic.bin`（Magic 不正）
  - `truncated.bin`（ヘッダー途中終端）
- [ ] `tests/fixtures/README.md` に UE バージョンと生成手順を記載
- [ ] Scanner 統合テスト（各フィクスチャの AssetType・dependencies を検証）
- [ ] Parser 単体テスト（error cases はインラインバイト列で検証）

---

### 4. `crates/asset-db` 実装

- [ ] スキーマ作成（`open()` 時に `CREATE TABLE IF NOT EXISTS` で自動生成）
  - `assets` テーブル（id / asset_path / file_path / asset_type / file_size / last_modified）
  - `dependencies` テーブル（from_id / to_path）
  - 3 インデックス（last_modified / asset_type / dependencies.to_path）
- [ ] `AssetRecord` 構造体定義（DB 行に対応）
- [ ] `AssetFilter` 構造体定義（asset_type / min_size / max_size / path_pattern）
- [ ] `AssetDb` 全 API 実装
  - `open(db_path)` → DB 作成 or オープン
  - `filter_changed(files: &[(PathBuf, u64)])` → 変更/新規ファイルのみ返す
  - `upsert_asset(meta)` → トランザクション外から呼ばれる前提（呼び出し側がトランザクション管理）
  - `delete_asset(asset_path)` → CASCADE 削除
  - `all_known_files()` → 削除検知用
  - `replace_dependencies(from_id, to_paths)` → 差し替え upsert
  - `all_edges()`・`all_assets()` → グラフ構築用（Phase 2 で使用）
  - `get_asset(asset_path)` → Option<AssetRecord>
  - `find_assets(filter)` → Vec<AssetRecord>
- [ ] `asset-db` 単体テスト（CRUD 往復・filter_changed の 3 ケース・find_assets フィルタ）

---

### 5. `crates/cli` — `scan` コマンド実装

- [ ] clap による CLI エントリポイント構築（コマンドツリー設計）
- [ ] DB パス解決ロジック（`<project_dir>/.uasset-lens/uasset-lens.db`、`--db` フラグ対応）
- [ ] Content root 解決ルール
  - `<project_dir>/Content/` が存在 → `content_root = Content/`
  - 存在しない → `content_root = <project_dir>`
- [ ] `scan` コマンドハンドラ
  - `walkdir` で `.uasset` / `.umap` を再帰列挙（`exclude_paths` フィルタは Phase 3 で追加）
  - 差分スキャン: `db.filter_changed()` で変更ファイルのみ絞り込み → `scanner::scan_files()`
  - `--full-scan` フラグ: 全ファイルを `scan_files()` に渡す
  - バッチ upsert（`rusqlite::Transaction` で一括コミット）
  - 削除検知: `db.all_known_files()` と walkdir 結果を比較
  - 削除確認プロンプト（`[y/N]`）と `-y` / `--yes` フラグ
- [ ] テキスト出力実装（`docs/rules/cli-output.md` 準拠）
  - 進捗 → stderr、結果サマリー → stdout
- [ ] JSON 出力実装（`--format json`、`docs/specs/cli-design.md` の JSON スキーマに準拠）
- [ ] exit codes 実装（0 / 1 / 2）
- [ ] `scan` 未実行時のエラーメッセージ（他コマンドから DB 未存在を検知した場合）
- [ ] 共通フラグ実装（`--format`・`--db`・`-y`）

---

### 6. `apps/uasset-lens-cli` — バイナリ作成

- [ ] `main.rs`（数行、`cli::run()` を呼ぶだけ）
- [ ] `Cargo.toml`（`cli` クレートへの依存のみ）
- [ ] リリースビルド確認（`cargo build --release`）

---

## 完了条件

### 機能要件

- [ ] `uasset-lens scan ./Project` が実際の UE5 プロジェクトで正常動作する
- [ ] 差分スキャン: 変更なし再実行が 1 秒未満で完了する
- [ ] `--full-scan` フラグが正しく機能する（mtime 無視で全ファイル再解析）
- [ ] ディスク上から消えたアセットが検知され、削除確認プロンプトが表示される
- [ ] `-y` フラグで確認なし自動削除が動作する
- [ ] テキスト出力が `docs/specs/cli-design.md` の `scan` コマンド仕様に一致する
- [ ] `--format json` 出力が仕様の JSON スキーマに一致する
- [ ] 存在しないディレクトリを渡すと exit code `2` で終了する

### テスト要件

- [ ] `cargo test --workspace` がすべてのプラットフォーム（Windows / macOS / Linux）でパスする
- [ ] `tests/fixtures/` の全フィクスチャファイルが正しい `AssetType` で解析される
- [ ] `bad_magic.bin` が `ScanError::InvalidMagic` で skipped に入る
- [ ] `truncated.bin` が `ScanError::UnexpectedEof` で skipped に入る
- [ ] `AssetPath` バリデーション全ケースがテストされている
- [ ] `asset-db` の `filter_changed()` — 新規・変更あり・変更なし の 3 ケースがテストされている

### 品質要件

- [ ] `cargo clippy --workspace -- -D warnings` が警告ゼロでパスする
- [ ] `cargo fmt --check` がパスする（フォーマット統一）
- [ ] ライブラリクレートで `println!` / `eprintln!` を使っていない
- [ ] `unsafe` ブロックに `// SAFETY:` コメントがある（該当する場合）

### パフォーマンス要件（リリースビルド）

- [ ] フルスキャン 1,000 assets: `--full-scan` で 5 秒以内
- [ ] メモリ使用量: 1,000 assets スキャン時に 100 MB 未満

### ドキュメント要件

- [ ] `tests/fixtures/README.md` に UE バージョンと生成手順が記載されている
- [ ] `docs/specs/testing.md` の `.gitattributes` 設定が適用されている
