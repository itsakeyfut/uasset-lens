# Tech Stack

## Core

- Rust

## Target Unreal Engine Version

- UE5 only (5.1+)

## Target File Formats

- **Analysis targets**: development content (`.uasset` / `.umap`) only
- **Permanently out of scope**: IoStore format (`.utoc` / `.ucas`) — cooked / packaged builds are not supported and will not be added in future phases. However, the parser architecture is designed to be extensible.

## GUI

- egui

## Database

- SQLite (designed to be migratable to DuckDB in the future)

## .uasset Parser Implementation

- **All `.uasset` parsing logic is hand-written** (no third-party `.uasset` / UE parsing crates)
- General binary parsing utilities (`byteorder` + `Cursor`) are used freely
- Maximizes portfolio value as a binary parsing skill demonstration
- The Asset Registry (`AssetRegistry.bin`) is referenced only as a supplement (when present)

### Parsed Format (UE5)

`.uasset` is a binary file with the following structure:

```
FPackageFileSummary (file header)
 ├─ Magic Number      : 0x9E2A83C1
 ├─ LegacyFileVersion / FileVersionUE5
 ├─ Name Table        : all strings used within the package
 ├─ Import Table      : Hard References to other assets (FObjectImport)
 ├─ Export Table      : objects defined by this package (FObjectExport)
 └─ Soft Reference    : Soft Object Paths embedded in property data
```

### Implementation Depth per Phase

- Phase 1: header + Name Table + Import/Export Table (minimum needed for dependency analysis)
- Phase 2: Export data property analysis (reading Blueprint nodes)
- Phase 3+: full Soft Reference analysis

## Graph Processing

- petgraph

## CLI

- clap

## Scan Mode

- **Default**: delta scan (mtime-based)
  - Stores each file's last-modified time (mtime) in the DB
  - Re-parses only files whose mtime has changed since the last scan
- **`--full-scan` option**: forces re-analysis of all assets

Items stored in DB per scan (scanner):
- `file_path`
- `last_modified` (mtime)

## CLI Output Format

- Text (default, human-readable)
- JSON (`--format json` option, for CI / tool integration)

## Serialization

- serde

## Parallelism

| Use case | Library |
|----------|---------|
| CPU-bound work (scanning and analysis) | `rayon` |
| Async I/O and event-driven (Watch Mode, future HTTP integration) | `tokio` |

## Error Handling

- Application layer (CLI / GUI): `anyhow`
- Library layer (each crate): `thiserror`

## Logging and Tracing

- `tracing` (structured logging and span information)

## Other Libraries

| Use case | Library |
|----------|---------|
| Recursive directory walk (used by `cli` crate) | `walkdir` |
| File system watching (Watch Mode) | `notify` |
| TOML parsing (config file) | `toml` |
| HTML templates (Report Generator) | `askama` |
| File hashing (future duplicate detection) | `xxhash-rust` |
