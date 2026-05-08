# `crates/scanner` — ExportTable parser and `scan_files()`

## Summary

Implement `parser/export.rs` for `AssetType` detection and the top-level `scan_files()`
function that ties all parsers together with rayon parallel scanning.
Complete when all 6 valid fixtures scan with the correct `AssetType` and `bad_magic.bin`
appears in `ScanResult.skipped`.

## Design Notes

**AssetType detection from ExportTable:**

Read the first `FObjectExport`. Its `ClassIndex` field (i32) indexes into the import table
(negative value → import entry at `abs(ClassIndex) - 1`) or export table (positive → local).
Resolve the class name via the import/name tables, then map to `AssetType`:

| Class name string | AssetType |
|---|---|
| `"Blueprint"` | `Blueprint` |
| `"AnimBlueprint"` | `AnimBlueprint` |
| `"WidgetBlueprint"` | `UserWidget` |
| `"Texture2D"` | `Texture2D` |
| `"StaticMesh"` | `StaticMesh` |
| `"SkeletalMesh"` | `SkeletalMesh` |
| `"Material"` | `Material` |
| `"MaterialInstanceConstant"` | `MaterialInstance` |
| `"ObjectRedirector"` | `ObjectRedirector` |
| anything else | `Unknown(class_name)` |

`.umap` extension → always `AssetType::World`, skip export table lookup.

**`scan_files()` design:**

```rust
pub fn scan_files(files: &[PathBuf], content_root: &Path) -> ScanResult
```

- `rayon::par_iter()` over `files`
- Per file: `std::fs::read()` → parse chain → build `AssetMetadata`
- Errors partition into `ScanResult.skipped` (never abort the whole scan)
- `tracing::warn!` for each skipped file

**Types to define:**

```rust
pub struct AssetMetadata {
    pub asset_path: AssetPath,
    pub file_path: PathBuf,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub last_modified: u64,   // Unix timestamp from file metadata
    pub dependencies: Vec<AssetPath>,
}

pub struct ScanResult {
    pub assets: Vec<AssetMetadata>,
    pub skipped: Vec<SkippedFile>,
}

pub struct SkippedFile {
    pub file_path: PathBuf,
    pub reason: ScanError,
}
```

## Requirements

- [ ] Implement `parse_export_table(data, offset, count, name_table, import_entries) -> Result<AssetType, ScanError>` (reads first export's ClassIndex and maps to AssetType)
- [ ] Force `AssetType::World` for `.umap` files before calling export parser
- [ ] Define `AssetMetadata`, `ScanResult`, and `SkippedFile` structs
- [ ] Implement `scan_files(files: &[PathBuf], content_root: &Path) -> ScanResult` using `rayon::par_iter()`
- [ ] Per-file: read file bytes + metadata (size, mtime), run parse chain, build `AssetMetadata`
- [ ] Partition results: parse errors go to `ScanResult.skipped` with `tracing::warn!`, not early return
- [ ] Integration test: scan all 6 valid fixtures → each returns expected `AssetType`
- [ ] Integration test: `bad_magic.bin` appears in `ScanResult.skipped` with `ScanError::InvalidMagic`
- [ ] Integration test: `truncated.bin` appears in `ScanResult.skipped` with `ScanError::UnexpectedEof`

## Related

- Depends on: #7 (import_table), #4 (fixture files)
- Next: #9 — asset-db schema
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 3-3, `docs/rules/binary-parser.md`
