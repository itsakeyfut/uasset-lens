# crate 詳細設計（Phase 1）

## `shared` crate 詳細設計

### 設計方針

- 型定義・エラー型のみを提供する。ロジックは各クレートに持たせる。
- 全クレートから依存されるため、依存クレートは最小限に抑える（`thiserror` + `serde` のみ）。

### ファイル構成

```
crates/shared/src/
  lib.rs
  asset_path.rs    # AssetPath・AssetPathError
  asset_type.rs    # AssetType enum
  version.rs       # FPackageVersion
```

### `AssetPath` 型

UE の Asset パス（例: `/Game/Characters/BP_Player`）を表す newtype。

```rust
pub struct AssetPath(String);

impl AssetPath {
    /// UE パス形式を検証してコンストラクト。不正な場合は Err を返す。
    pub fn new(s: impl Into<String>) -> Result<Self, AssetPathError>;
    pub fn as_str(&self) -> &str;
    /// オブジェクト名サフィックスを除去してパッケージ名を返す
    /// 例: "/Game/Chars/BP_Player.BP_Player" → "/Game/Chars/BP_Player"
    pub fn package_name(&self) -> &str;
    /// ファイルシステムの絶対パスからゲームパスへ変換
    /// 例: content_root="/Project/Content", file="/Project/Content/Chars/BP_Player.uasset"
    ///     → "/Game/Chars/BP_Player"
    pub fn from_fs_path(content_root: &Path, file_path: &Path) -> Result<Self, AssetPathError>;
}
```

バリデーション規則:

- 空文字列は不可
- 先頭が `/` であること
- ファイル拡張子（`.uasset`・`.umap`）を含まないこと

```rust
#[derive(Debug, thiserror::Error)]
pub enum AssetPathError {
    #[error("asset path is empty")]
    Empty,
    #[error("asset path must start with '/'")]
    MissingLeadingSlash,
    #[error("asset path contains invalid character: {0:?}")]
    InvalidCharacter(char),
    #[error("file is not under the content root")]
    NotUnderContentRoot,
}
```

### `AssetType` enum

Phase 1 の解析に必要な主要型を列挙し、未知クラスは `Unknown(String)` でクラス名を保持する。

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetType {
    // Blueprint 系
    Blueprint, BlueprintInterface, AnimBlueprint, UserWidget,
    // Mesh 系
    StaticMesh, SkeletalMesh,
    // Texture 系
    Texture2D,
    // Material 系
    Material, MaterialInstance, MaterialFunction,
    // Audio 系
    SoundWave, SoundCue,
    // Animation 系
    AnimSequence, AnimMontage,
    // データ系
    DataTable, DataAsset,
    // レベル系
    World,                  // .umap ファイル
    // 特殊
    ObjectRedirector,       // Redirector 解析に必須
    // 未知（UE クラス名をそのまま保持）
    Unknown(String),
}
```

型の決定方法:

- `.umap` ファイルは問答無用で `World` とみなす
- `.uasset` は Export Table の最初の Export が持つクラス名（Import Table 経由で解決）から決定する

### `FPackageVersion` 型

`.uasset` ヘッダーから読み取るバージョン情報。

```rust
#[derive(Debug, Clone, Copy)]
pub struct FPackageVersion {
    pub legacy_version: i32,    // UE5 では -8
    pub file_version_ue4: u32,  // deprecated（UE5 では 0）
    pub file_version_ue5: u32,  // UE5 固有バージョン番号
}

impl FPackageVersion {
    /// UE5 形式かどうか（legacy_version == -8 かつ file_version_ue5 > 0）
    pub fn is_ue5(&self) -> bool;
}
```

### `shared` crate の依存クレート

```toml
[dependencies]
thiserror = { workspace = true }
serde     = { workspace = true, features = ["derive"] }
```

---

## `asset-db` crate 詳細設計

### 設計方針

- 差分スキャン（mtime ベース）のためのスキャン状態管理
- 依存関係の永続化（再スキャンなしでクエリできる）
- `/Game/` 下のプロジェクト Asset のみ対象（エンジン内部参照は保存しない）

### スキーマ

```sql
CREATE TABLE assets (
    id            INTEGER PRIMARY KEY,
    asset_path    TEXT    NOT NULL UNIQUE,  -- /Game/Characters/BP_Player
    file_path     TEXT    NOT NULL UNIQUE,  -- ファイルシステム絶対パス
    asset_type    TEXT    NOT NULL,         -- シリアライズされた AssetType
    file_size     INTEGER NOT NULL,         -- バイト数
    last_modified INTEGER NOT NULL          -- Unix タイムスタンプ（mtime）
);

CREATE TABLE dependencies (
    from_id  INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    to_path  TEXT    NOT NULL,              -- 参照先の asset_path（未登録でも可）
    PRIMARY KEY (from_id, to_path)
);
```

### インデックス

```sql
-- 差分スキャン: mtime が変化したファイルを高速に取得
CREATE INDEX idx_assets_last_modified ON assets(last_modified);

-- 型フィルタ検索（find コマンド向け）
CREATE INDEX idx_assets_type ON assets(asset_type);

-- 逆引き依存（Impact Analyzer 向け）: "この Asset を参照している Asset は？"
CREATE INDEX idx_dependencies_to_path ON dependencies(to_path);
```

### エンジン内部参照のフィルタリング

依存関係の保存時、以下のプレフィックスを持つ参照はスキップする。

| プレフィックス | 例 | 理由 |
|---|---|---|
| `/Script/` | `/Script/Engine.StaticMesh` | エンジンクラス定義 |
| `/Engine/` | `/Engine/Content/T_DefaultNormal` | エンジン内蔵コンテンツ |

`/Game/` で始まる参照のみ保存する。

### 型定義

```rust
/// assets テーブルの 1 行に対応する型（scanner の AssetMetadata とは異なり dependencies を持たない）
pub struct AssetRecord {
    pub id:            i64,
    pub asset_path:    AssetPath,
    pub file_path:     PathBuf,
    pub asset_type:    AssetType,
    pub file_size:     u64,
    pub last_modified: u64,
}

/// find_assets() のフィルタ条件（全フィールドが Option であり、None は条件なしを意味する）
pub struct AssetFilter {
    pub asset_type:   Option<AssetType>,
    pub min_size:     Option<u64>,        // バイト数下限（--larger-than）
    pub max_size:     Option<u64>,        // バイト数上限（--smaller-than）
    pub path_pattern: Option<String>,     // glob パターン（--path）
}
```

### クエリ API

```rust
impl AssetDb {
    pub fn open(db_path: &Path) -> Result<Self>;

    // --- 差分スキャン ---
    /// ファイル一覧（パス + 現在の mtime）のうち DB に存在しない / mtime が変化したものだけを返す
    /// cli が walkdir で全ファイルを列挙した後にこのメソッドで絞り込む
    pub fn filter_changed(&self, files: &[(PathBuf, u64)]) -> Result<Vec<PathBuf>>;
    /// Asset を upsert（差分スキャンの書き込み）
    pub fn upsert_asset(&self, meta: &AssetMetadata) -> Result<i64>;
    /// DB には存在するがファイルが消えた Asset を削除
    pub fn delete_asset(&self, asset_path: &AssetPath) -> Result<()>;
    /// DB に登録されている全ファイルパス + mtime を返す（削除検知用）
    pub fn all_known_files(&self) -> Result<Vec<(PathBuf, u64)>>;

    // --- 依存関係 ---
    /// Asset の依存関係を一括 upsert（古いものは削除して置き換え）
    pub fn replace_dependencies(&self, from_id: i64, to_paths: &[AssetPath]) -> Result<()>;
    /// 全エッジ（from_path, to_path）を返す（DependencyGraph 構築用）
    pub fn all_edges(&self) -> Result<Vec<(AssetPath, AssetPath)>>;

    // --- クエリ ---
    /// asset_path で Asset を取得
    pub fn get_asset(&self, asset_path: &AssetPath) -> Result<Option<AssetRecord>>;
    /// 全 Asset を返す（DependencyGraph のノード構築用）
    pub fn all_assets(&self) -> Result<Vec<AssetRecord>>;
    /// この Asset が参照している Asset 一覧（順方向）
    pub fn get_dependencies(&self, asset_path: &AssetPath) -> Result<Vec<AssetPath>>;
    /// この Asset を参照している Asset 一覧（逆方向）
    pub fn get_reverse_dependencies(&self, asset_path: &AssetPath) -> Result<Vec<AssetPath>>;
    /// 型・サイズ・パターンでフィルタ検索（find コマンド用）
    pub fn find_assets(&self, filter: &AssetFilter) -> Result<Vec<AssetRecord>>;
}
```

### `asset-db` crate の依存クレート

```toml
[dependencies]
rusqlite  = { workspace = true, features = ["bundled"] }  # SQLite を静的リンク
shared    = { path = "../shared" }
thiserror = { workspace = true }
```

---

## `scanner` crate 詳細設計

### 設計方針

- `.uasset` / `.umap` 解析ロジックは完全自前実装（既存の UE 解析クレートは不使用）
- バイナリ解析には `byteorder` + `Cursor` を使用する
- ファイル単位で `rayon` による CPU 並列スキャンを行う
- 破損ファイル・未知バージョンは警告ログを出してスキップし、スキャン全体は継続する

### ファイル構成

```
crates/scanner/src/
  lib.rs
  scanner.rs      # 並列スキャンのエントリポイント（ファイルリスト受け取り）
  parser/
    mod.rs
    header.rs     # FPackageFileSummary パース
    name_table.rs # Name Table パース
    import.rs     # Import Table（Hard Reference）パース
    export.rs     # Export Table（AssetType 判定）パース
  error.rs
```

### `AssetMetadata` 構造体

scanner が 1 ファイルの解析結果として返す型。

```rust
pub struct AssetMetadata {
    pub asset_path:    AssetPath,         // /Game/Characters/BP_Player
    pub file_path:     PathBuf,           // ファイルシステム絶対パス
    pub asset_type:    AssetType,         // Export Table から決定
    pub file_size:     u64,               // バイト数
    pub last_modified: u64,               // Unix タイムスタンプ（mtime）
    pub dependencies:  Vec<AssetPath>,    // Import Table 由来の Hard Reference
}
```

### パースパイプライン（1 ファイル）

```
ファイル読み込み
  → Magic Number 検証（0x9E2A83C1）
  → FPackageFileSummary パース
      └─ LegacyFileVersion / FileVersionUE5 確認
  → Name Table パース（オフセット・カウントはヘッダーから取得）
  → Import Table パース
      └─ /Script/ / /Engine/ プレフィックスをフィルタリング
      └─ /Game/ のみ AssetPath に変換して dependencies に追加
  → Export Table の先頭 Export からクラス名を取得
      └─ Name Table を引いて AssetType に変換
  → AssetMetadata を返す
```

### 公開 API

scanner はファイルの走査・フィルタリングを一切行わない純粋なパーサー層。
ディレクトリウォーク・`exclude_paths` フィルタ・差分判定はすべて `cli` が担い、
解析すべきファイルのリストを渡してくる。

```rust
/// 指定されたファイル一覧を rayon で並列パースして AssetMetadata を返す
/// content_root: /Game/ パス変換のためのベースパス
pub fn scan_files(
    files: &[PathBuf],
    content_root: &Path,
) -> ScanResult;

pub struct ScanResult {
    pub assets:  Vec<AssetMetadata>,
    pub skipped: Vec<SkippedFile>,
}

pub struct SkippedFile {
    pub path:   PathBuf,
    pub reason: ScanError,
}
```

**差分スキャンのフロー（cli 側）**:

```
// フルスキャン（--full-scan）
let all_files = walkdir(content_root, &exclude_paths);
let results   = scanner::scan_files(&all_files, content_root);

// 差分スキャン（デフォルト）
let all_files_with_mtime: Vec<(PathBuf, u64)> = walkdir(content_root, &exclude_paths);
let changed = db.filter_changed(&all_files_with_mtime)?;
let results = scanner::scan_files(&changed, content_root);
```

### エラー型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("invalid magic number: {0:#x}")]
    InvalidMagic(u32),
    #[error("unsupported file version: legacy={0}, ue5={1}")]
    UnsupportedVersion(i32, u32),
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

### `scanner` crate の依存クレート

`walkdir` は `cli` crate に移動（scanner はファイルリストを受け取るだけでウォークしない）。

```toml
[dependencies]
shared    = { path = "../shared" }
byteorder = { workspace = true }
rayon     = { workspace = true }
thiserror = { workspace = true }
tracing   = { workspace = true }
```

---

## `dependency-graph` crate 詳細設計

### 設計方針

- 純粋なグラフ計算層。DB や IO に依存しない（`shared` + `petgraph` のみ）。
- データの取得は呼び出し側（`cli`）が担い、グラフはエッジリストから構築される。
- `dead-asset-detector`・`impact-analyzer`・`redirector-analyzer` はこのクレートを利用する。

### 型定義

```rust
/// グラフのノードデータ。パスと型情報を持つ
pub struct AssetNode {
    pub path:       AssetPath,
    pub asset_type: AssetType,
}

/// find_impact() の戻り値。直接参照と推移的参照を分離して返す
pub struct ImpactResult {
    /// 1 ホップで直接参照している Asset
    pub direct:     Vec<AssetPath>,
    /// 2 ホップ以上の推移的参照（direct は含まない）
    pub transitive: Vec<AssetPath>,
}
```

### 内部構造

```rust
pub struct DependencyGraph {
    /// 有向グラフ（A → B は「A が B を参照する」を意味する）
    graph: DiGraph<AssetNode, ()>,
    /// O(1) ルックアップ用インデックス
    index: HashMap<AssetPath, NodeIndex>,
}
```

### 公開 API（Phase 1）

```rust
impl DependencyGraph {
    /// ノード一覧 + エッジリストからグラフを構築
    /// nodes: DB の全 Asset（AssetType 情報を持つ）
    /// edges: (from_path, to_path) の依存関係
    pub fn build(
        nodes: impl IntoIterator<Item = AssetNode>,
        edges: impl IntoIterator<Item = (AssetPath, AssetPath)>,
    ) -> Self;

    /// 全ノードを AssetNode のイテレータとして返す
    pub fn nodes(&self) -> impl Iterator<Item = &AssetNode>;

    /// 指定 Asset の入次数（何個の Asset から参照されているか）を返す
    pub fn in_degree(&self, path: &AssetPath) -> usize;

    /// 循環依存するグループをすべて列挙（Tarjan の強連結成分分解）
    pub fn find_cycles(&self) -> Vec<Vec<AssetPath>>;

    /// 指定 Asset を削除したときに壊れる Asset を直接参照・推移的参照に分けて返す
    pub fn find_impact(&self, target: &AssetPath) -> ImpactResult;
}
```

### アルゴリズム

| メソッド | アルゴリズム | petgraph API |
|---|---|---|
| `find_cycles` | Tarjan の強連結成分分解（SCC） | `petgraph::algo::tarjan_scc` |
| `find_impact` | 逆方向グラフ上での BFS（1 ホップで direct、残りを transitive） | `Reversed` + `Bfs` |
| `in_degree` | ノードの入次数 | `graph.edges_directed(node, Incoming).count()` |

### `dependency-graph` crate の依存クレート

```toml
[dependencies]
shared   = { path = "../shared" }
petgraph = { workspace = true }
```

---

## `dead-asset-detector` crate 詳細設計

### 設計方針

- `DependencyGraph` を受け取り、入次数 0 のノード（どの Asset からも参照されていない Asset）を列挙する
- DB / IO に依存しない純粋関数層

### 公開 API

```rust
/// グラフ内で入次数が 0 の Asset（孤立ノード）を列挙する
/// 戻り値は AssetPath のみ。型・サイズ等の詳細は呼び出し側が DB から補完する
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>;
```

### 実装メモ

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph.nodes()
        .filter(|node| graph.in_degree(&node.path) == 0)
        .map(|node| node.path.clone())
        .collect()
}
```

### `dead-asset-detector` crate の依存クレート

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `impact-analyzer` crate 詳細設計

### Phase 1 スコープ

Phase 1 では `dependency-graph.find_impact()` が `ImpactResult` を返すため、
`cli` は `dependency_graph.find_impact(target)` を直接呼び出す。
この crate は Phase 2 以降の拡張に備えたプレースホルダーとして存在する。

### Phase 2 以降で追加予定の機能

- リネーム安全性チェック（Redirector が補完できるか判定）
- Soft Reference を含めた影響範囲の精度向上
- 間接影響の深さ制限オプション

### `impact-analyzer` crate の依存クレート

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `redirector-analyzer` crate 詳細設計

### 設計方針

- `DependencyGraph` のノードを走査し `ObjectRedirector` 型の Asset を列挙する
- DB / IO に依存しない純粋関数層
- Phase 1 スコープ: 検出・列挙のみ。redirect 先の解決は Phase 2 以降

### 公開 API

```rust
/// グラフ内の ObjectRedirector 型 Asset を列挙する
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>;
```

### 実装メモ

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph.nodes()
        .filter(|node| node.asset_type == AssetType::ObjectRedirector)
        .map(|node| node.path.clone())
        .collect()
}
```

### `redirector-analyzer` crate の依存クレート

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `cli` crate の依存クレート（参考）

```toml
[dependencies]
shared               = { path = "../shared" }
scanner              = { path = "../scanner" }
asset-db             = { path = "../asset-db" }
dependency-graph     = { path = "../dependency-graph" }
dead-asset-detector  = { path = "../dead-asset-detector" }
impact-analyzer      = { path = "../impact-analyzer" }
redirector-analyzer  = { path = "../redirector-analyzer" }
clap      = { workspace = true }
anyhow    = { workspace = true }
tracing   = { workspace = true }
walkdir   = { workspace = true }   # ディレクトリウォーク（scanner から移動）
toml      = { workspace = true }   # .uasset-lens.toml 読み込み
serde     = { workspace = true }
```
