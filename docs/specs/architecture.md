# Architecture

## Overall Architecture

```text
uasset-lens/
 ├─ crates/
 │   ├─ scanner               # .uasset scan and metadata extraction
 │   ├─ asset-db              # SQLite-backed asset index management
 │   ├─ dependency-graph      # Hard/Soft Reference analysis and graph construction
 │   ├─ impact-analyzer       # Delete / rename impact analysis
 │   ├─ redirector-analyzer   # Redirector detection and analysis
 │   ├─ dead-asset-detector   # Unused asset detection
 │   ├─ duplicate-detector    # Duplicate asset detection
 │   ├─ bp-analyzer           # Blueprint static analysis
 │   ├─ level-analyzer        # Level / World Partition analysis
 │   ├─ material-analyzer     # Material complexity analysis
 │   ├─ lint-engine           # Linter rule engine
 │   ├─ budget-tracker        # Performance budget management
 │   ├─ git-diff              # Blueprint / asset diff analysis
 │   ├─ watcher               # File system watching (Watch Mode)
 │   ├─ reporter              # HTML / Markdown report generation
 │   ├─ dashboard             # egui GUI dashboard
 │   ├─ cli                   # CLI command definitions (clap)
 │   └─ shared                # Common types and utilities
 │
 ├─ apps/
 │   ├─ uasset-lens-cli
 │   └─ uasset-lens-desktop
 │
 └─ docs/
```

---

## Phase 1 Workspace Structure

### Crate List (Phase 1)

| Crate | Kind | Role |
|-------|------|------|
| `crates/shared` | lib | Common type definitions and error types (`AssetPath`, `AssetType`, `FPackageVersion`) |
| `crates/scanner` | lib | `.uasset` binary parser and metadata extraction |
| `crates/asset-db` | lib | SQLite-backed asset index management and delta scanning |
| `crates/dependency-graph` | lib | Dependency graph construction and circular dependency detection |
| `crates/dead-asset-detector` | lib | Unused and isolated asset detection |
| `crates/impact-analyzer` | lib | Delete / rename impact scope analysis (Phase 1: thin wrapper over `dependency-graph.find_impact()`; Phase 2 adds rename-safety checks and Soft Reference analysis) |
| `crates/redirector-analyzer` | lib | Redirector detection and analysis |
| `crates/cli` | lib | clap command definitions, handler logic, output formatting, directory walk, config file loading |
| `apps/uasset-lens-cli` | bin | Entry point (`main.rs` only) |

### Crate Dependency Graph

```text
shared
  ├── scanner
  ├── asset-db
  ├── dependency-graph
  │     ├── dead-asset-detector
  │     └── impact-analyzer
  └── redirector-analyzer

cli ← shared
    ← scanner
    ← asset-db
    ← dependency-graph
    ← dead-asset-detector
    ← impact-analyzer
    ← redirector-analyzer

apps/uasset-lens-cli ← cli (main.rs only)
```

Dependency rules:
- `shared` depends on nothing (bottom of the dependency graph)
- `scanner`, `asset-db`, and `redirector-analyzer` depend only on `shared`
- `dependency-graph` depends only on `shared` and `petgraph` (pure graph computation, no DB or I/O)
- `dead-asset-detector` and `impact-analyzer` depend on `dependency-graph`
- `cli` depends on all library crates; it owns directory walking (`walkdir`) and config file loading (`toml`)
- `apps/uasset-lens-cli` depends only on `cli` (thin entry point)

### `cli` / `apps` Separation

| Package | Kind | Contents |
|---------|------|----------|
| `crates/cli` | library crate | clap command definitions, handler logic, output formatting |
| `apps/uasset-lens-cli` | binary crate | `main.rs` only (a few lines); delegates everything to `crates/cli` |

This design allows the future GUI (`apps/uasset-lens-desktop`) to reuse `crates/cli` logic.

### `shared` crate contents

Type definitions and error types only. Utility functions belong in their respective crates.

| File | Contents |
|------|----------|
| `asset_path.rs` | `AssetPath` type |
| `asset_type.rs` | `AssetType` enum |
| `error.rs` | Common error types (thiserror-based) |
| `version.rs` | `FPackageVersion` |

### Workspace Configuration

- Dependency crate versions are centrally managed in `[workspace.dependencies]`
- All crates use `edition = "2021"`
- Use `resolver = "2"` (Rust 2021 default)

---

## Internal Data Model

### Graph Model

```text
Asset -> Asset
Blueprint -> Component
Material -> Texture
Blueprint -> Blueprint
```

### Rationale for using a Database

Enables:

- Fast search
- Trend analysis
- Historical diff
- Query system
- Large-scale analysis

### Example Query

```sql
SELECT *
FROM blueprint_dependencies
WHERE circular = true;
```
