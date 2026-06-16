# Crate Detailed Design (Phase 1)

## `shared` crate

### Design

- Provides type definitions and error types only. Logic belongs in each crate.
- Keep dependencies minimal (only `thiserror` + `serde`) since every crate depends on this one.

### File Structure

```
crates/shared/src/
  lib.rs
  asset_path.rs    # AssetPath, AssetPathError
  asset_type.rs    # AssetType enum
  version.rs       # FPackageVersion
```

### `AssetPath` type

A newtype representing a UE asset path (e.g. `/Game/Characters/BP_Player`).

```rust
pub struct AssetPath(String);

impl AssetPath {
    /// Validates and constructs from a UE path string. Returns Err for invalid input.
    pub fn new(s: impl Into<String>) -> Result<Self, AssetPathError>;
    pub fn as_str(&self) -> &str;
    /// Strips the object-name suffix and returns the package name.
    /// e.g. "/Game/Chars/BP_Player.BP_Player" → "/Game/Chars/BP_Player"
    pub fn package_name(&self) -> &str;
    /// Converts a filesystem absolute path to a game path.
    /// e.g. content_root="/Project/Content", file="/Project/Content/Chars/BP_Player.uasset"
    ///     → "/Game/Chars/BP_Player"
    pub fn from_fs_path(content_root: &Path, file_path: &Path) -> Result<Self, AssetPathError>;
}
```

Validation rules:

- Empty string is not allowed
- Must start with `/`
- Must not contain a file extension (`.uasset` / `.umap`)

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

Enumerates the primary types needed for Phase 1 analysis. Unknown classes are retained via `Unknown(String)`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetType {
    // Blueprint family
    Blueprint, BlueprintInterface, AnimBlueprint, UserWidget,
    // Mesh family
    StaticMesh, SkeletalMesh,
    // Texture family
    Texture2D,
    // Material family
    Material, MaterialInstance, MaterialFunction,
    // Audio family
    SoundWave, SoundCue,
    // Animation family
    AnimSequence, AnimMontage,
    // Data family
    DataTable, DataAsset,
    // Level family
    World,                  // .umap files
    // Special
    ObjectRedirector,       // Required for redirector analysis
    // Unknown (preserves the raw UE class name)
    Unknown(String),
}
```

Type determination:
- `.umap` files are always treated as `World`
- For `.uasset` files, the type is derived from the class name of the first Export in the Export Table (resolved via the Import Table)

### `FPackageVersion` type

Version information read from the `.uasset` header.

```rust
#[derive(Debug, Clone, Copy)]
pub struct FPackageVersion {
    pub legacy_version: i32,    // -8 in UE5
    pub file_version_ue4: u32,  // deprecated (0 in UE5)
    pub file_version_ue5: u32,  // UE5-specific version number
}

impl FPackageVersion {
    /// Returns true for UE5 files (legacy_version == -8 and file_version_ue5 > 0)
    pub fn is_ue5(&self) -> bool;
}
```

### `shared` crate dependencies

```toml
[dependencies]
thiserror = { workspace = true }
serde     = { workspace = true, features = ["derive"] }
```

---

## `asset-db` crate

### Design

- Manages scan state for delta scanning (mtime-based)
- Persists dependency relationships (queryable without re-scanning)
- Scope: project assets under `/Game/` only (engine-internal references are not stored)

### Schema

```sql
CREATE TABLE assets (
    id            INTEGER PRIMARY KEY,
    asset_path    TEXT    NOT NULL UNIQUE,  -- /Game/Characters/BP_Player
    file_path     TEXT    NOT NULL UNIQUE,  -- filesystem absolute path
    asset_type    TEXT    NOT NULL,         -- serialized AssetType
    file_size     INTEGER NOT NULL,         -- bytes
    last_modified INTEGER NOT NULL          -- Unix timestamp (mtime)
);

CREATE TABLE dependencies (
    from_id  INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    to_path  TEXT    NOT NULL,              -- asset_path of referenced asset (may not exist in DB)
    PRIMARY KEY (from_id, to_path)
);
```

### Indexes

```sql
-- Delta scan: quickly retrieve files whose mtime has changed
CREATE INDEX idx_assets_last_modified ON assets(last_modified);

-- Type filter search (for the find command)
CREATE INDEX idx_assets_type ON assets(asset_type);

-- Reverse dependency lookup (for Impact Analyzer): "which assets reference this asset?"
CREATE INDEX idx_dependencies_to_path ON dependencies(to_path);
```

### Engine-internal reference filtering

When saving dependencies, skip references with the following prefixes:

| Prefix | Example | Reason |
|--------|---------|--------|
| `/Script/` | `/Script/Engine.StaticMesh` | Engine class definitions |
| `/Engine/` | `/Engine/Content/T_DefaultNormal` | Engine built-in content |

Only references starting with `/Game/` are stored.

### Type definitions

```rust
/// Corresponds to one row in the assets table (does not include dependencies, unlike AssetMetadata)
pub struct AssetRecord {
    pub id:            i64,
    pub asset_path:    AssetPath,
    pub file_path:     PathBuf,
    pub asset_type:    AssetType,
    pub file_size:     u64,
    pub last_modified: u64,
}

/// Filter criteria for find_assets() — all fields are Option; None means no constraint
pub struct AssetFilter {
    pub asset_type:   Option<AssetType>,
    pub min_size:     Option<u64>,        // bytes lower bound (--larger-than)
    pub max_size:     Option<u64>,        // bytes upper bound (--smaller-than)
    pub path_pattern: Option<String>,     // glob pattern (--path)
}
```

### Query API

```rust
impl AssetDb {
    pub fn open(db_path: &Path) -> Result<Self>;

    // --- Delta scanning ---
    /// From a list of (path, current mtime) pairs, returns only those that are new or have changed mtime in the DB
    /// The CLI enumerates all files with walkdir, then narrows the list with this method
    pub fn filter_changed(&self, files: &[(PathBuf, u64)]) -> Result<Vec<PathBuf>>;
    /// Upsert an asset (delta scan write)
    pub fn upsert_asset(&self, meta: &AssetMetadata) -> Result<i64>;
    /// Delete assets that exist in the DB but are no longer on disk
    pub fn delete_asset(&self, asset_path: &AssetPath) -> Result<()>;
    /// Returns all file paths + mtimes stored in the DB (for stale-record detection)
    pub fn all_known_files(&self) -> Result<Vec<(PathBuf, u64)>>;

    // --- Dependencies ---
    /// Bulk-upsert asset dependencies (replaces old entries)
    pub fn replace_dependencies(&self, from_id: i64, to_paths: &[AssetPath]) -> Result<()>;
    /// Returns all edges as (from_path, to_path) pairs (for DependencyGraph construction)
    pub fn all_edges(&self) -> Result<Vec<(AssetPath, AssetPath)>>;

    // --- Queries ---
    /// Look up an asset by its asset_path
    pub fn get_asset(&self, asset_path: &AssetPath) -> Result<Option<AssetRecord>>;
    /// Returns all assets (for DependencyGraph node construction)
    pub fn all_assets(&self) -> Result<Vec<AssetRecord>>;
    /// Returns the assets this asset depends on (forward direction)
    pub fn get_dependencies(&self, asset_path: &AssetPath) -> Result<Vec<AssetPath>>;
    /// Returns the assets that depend on this asset (reverse direction)
    pub fn get_reverse_dependencies(&self, asset_path: &AssetPath) -> Result<Vec<AssetPath>>;
    /// Filtered search by type, size, and path pattern (for the find command)
    pub fn find_assets(&self, filter: &AssetFilter) -> Result<Vec<AssetRecord>>;
}
```

### `asset-db` crate dependencies

```toml
[dependencies]
rusqlite  = { workspace = true, features = ["bundled"] }  # statically link SQLite
shared    = { path = "../shared" }
thiserror = { workspace = true }
```

---

## `scanner` crate

### Design

- All `.uasset` / `.umap` parsing logic is hand-written (no third-party UE parsing crates)
- Binary parsing uses `byteorder` + `Cursor`
- CPU-parallel scanning at the file level via `rayon`
- Corrupt files and unknown versions emit a warning log and are skipped; the overall scan continues

### File Structure

```
crates/scanner/src/
  lib.rs
  scanner.rs      # parallel scan entry point (accepts a file list)
  parser/
    mod.rs
    header.rs     # FPackageFileSummary parsing
    name_table.rs # Name Table parsing
    import.rs     # Import Table (Hard References) parsing
    export.rs     # Export Table (AssetType determination) parsing
  error.rs
```

### `AssetMetadata` struct

The type returned by the scanner for one parsed file.

```rust
pub struct AssetMetadata {
    pub asset_path:    AssetPath,         // /Game/Characters/BP_Player
    pub file_path:     PathBuf,           // filesystem absolute path
    pub asset_type:    AssetType,         // determined from Export Table
    pub file_size:     u64,               // bytes
    pub last_modified: u64,               // Unix timestamp (mtime)
    pub dependencies:  Vec<AssetPath>,    // Hard References from Import Table
}
```

### Parse Pipeline (per file)

```
Read file
  → Verify Magic Number (0x9E2A83C1)
  → Parse FPackageFileSummary
      └─ Check LegacyFileVersion / FileVersionUE5
  → Parse Name Table (offset and count from header)
  → Parse Import Table
      └─ Filter /Script/ and /Engine/ prefixes
      └─ Convert /Game/ paths to AssetPath and add to dependencies
  → Get class name from first Export in Export Table
      └─ Resolve via Name Table and convert to AssetType
  → Return AssetMetadata
```

### Public API

The scanner is a pure parsing layer — it never walks directories or applies filters.
Directory walking, `exclude_paths` filtering, and delta detection are all handled by `cli`,
which passes the list of files to scan.

```rust
/// Parses the given file list in parallel with rayon and returns AssetMetadata
/// content_root: base path used for /Game/ path conversion
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

**Delta scan flow (cli side)**:

```
// Full scan (--full-scan)
let all_files = walkdir(content_root, &exclude_paths);
let results   = scanner::scan_files(&all_files, content_root);

// Delta scan (default)
let all_files_with_mtime: Vec<(PathBuf, u64)> = walkdir(content_root, &exclude_paths);
let changed = db.filter_changed(&all_files_with_mtime)?;
let results = scanner::scan_files(&changed, content_root);
```

### Error type

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

### `scanner` crate dependencies

`walkdir` lives in the `cli` crate (scanner only accepts a file list, it does not walk directories).

```toml
[dependencies]
shared    = { path = "../shared" }
byteorder = { workspace = true }
rayon     = { workspace = true }
thiserror = { workspace = true }
tracing   = { workspace = true }
```

---

## `dependency-graph` crate

### Design

- Pure graph computation layer. No DB or I/O dependencies (only `shared` + `petgraph`).
- The caller (`cli`) fetches the data; the graph is built from an edge list.
- `dead-asset-detector`, `impact-analyzer`, and `redirector-analyzer` use this crate.

### Type definitions

```rust
/// Node data in the graph — holds path and type information
pub struct AssetNode {
    pub path:       AssetPath,
    pub asset_type: AssetType,
}

/// Return value of find_impact() — separates direct and transitive references
pub struct ImpactResult {
    /// Assets that directly reference the target (1 hop)
    pub direct:     Vec<AssetPath>,
    /// Assets that transitively reference the target (2+ hops, excludes direct)
    pub transitive: Vec<AssetPath>,
}
```

### Internal structure

```rust
pub struct DependencyGraph {
    /// Directed graph (A → B means "A references B")
    graph: DiGraph<AssetNode, ()>,
    /// O(1) lookup index
    index: HashMap<AssetPath, NodeIndex>,
}
```

### Public API (Phase 1)

```rust
impl DependencyGraph {
    /// Build the graph from a node list and an edge list
    /// nodes: all assets from the DB (with AssetType information)
    /// edges: (from_path, to_path) dependency pairs
    pub fn build(
        nodes: impl IntoIterator<Item = AssetNode>,
        edges: impl IntoIterator<Item = (AssetPath, AssetPath)>,
    ) -> Self;

    /// Returns all nodes as an iterator of AssetNode
    pub fn nodes(&self) -> impl Iterator<Item = &AssetNode>;

    /// Returns the in-degree of the given asset (number of assets that reference it)
    pub fn in_degree(&self, path: &AssetPath) -> usize;

    /// Enumerates all groups involved in circular dependencies (Tarjan SCC)
    pub fn find_cycles(&self) -> Vec<Vec<AssetPath>>;

    /// Returns the assets that would break if the given asset were deleted,
    /// split into direct and transitive references
    pub fn find_impact(&self, target: &AssetPath) -> ImpactResult;
}
```

### Algorithms

| Method | Algorithm | petgraph API |
|--------|-----------|--------------|
| `find_cycles` | Tarjan's strongly connected components (SCC) | `petgraph::algo::tarjan_scc` |
| `find_impact` | BFS on the reversed graph (1 hop = direct, rest = transitive) | `Reversed` + `Bfs` |
| `in_degree` | Node in-degree | `graph.edges_directed(node, Incoming).count()` |

### `dependency-graph` crate dependencies

```toml
[dependencies]
shared   = { path = "../shared" }
petgraph = { workspace = true }
```

---

## `dead-asset-detector` crate

### Design

- Accepts a `DependencyGraph` and enumerates nodes with in-degree 0 (assets not referenced by any other asset)
- Pure function layer — no DB or I/O dependencies

### Public API

```rust
/// Enumerates assets in the graph with in-degree 0 (isolated nodes)
/// Returns AssetPath only; the caller fetches type, size, etc. from the DB
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>;
```

### Implementation

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph.nodes()
        .filter(|node| graph.in_degree(&node.path) == 0)
        .map(|node| node.path.clone())
        .collect()
}
```

### `dead-asset-detector` crate dependencies

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `impact-analyzer` crate

### Phase 1 scope

In Phase 1, `dependency-graph.find_impact()` returns an `ImpactResult` directly,
so `cli` calls `dependency_graph.find_impact(target)` without going through this crate.
This crate exists as a placeholder for Phase 2+ extensions.

### Planned additions in Phase 2+

- Rename-safety check (can a Redirector compensate?)
- Higher-precision impact scope including Soft References
- Depth limit option for transitive impact

### `impact-analyzer` crate dependencies

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `redirector-analyzer` crate

### Design

- Walks `DependencyGraph` nodes and enumerates assets of type `ObjectRedirector`
- Pure function layer — no DB or I/O dependencies
- Phase 1 scope: detection and listing only. Redirect target resolution is Phase 2+

### Public API

```rust
/// Enumerates ObjectRedirector assets in the graph
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>;
```

### Implementation

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph.nodes()
        .filter(|node| node.asset_type == AssetType::ObjectRedirector)
        .map(|node| node.path.clone())
        .collect()
}
```

### `redirector-analyzer` crate dependencies

```toml
[dependencies]
shared            = { path = "../shared" }
dependency-graph  = { path = "../dependency-graph" }
```

---

## `cli` crate dependencies (reference)

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
walkdir   = { workspace = true }   # directory walk (moved from scanner)
toml      = { workspace = true }   # .uasset-lens.toml parsing
serde     = { workspace = true }
```
