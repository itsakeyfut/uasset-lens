# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

## [0.1.0] - 2026-05-20

### Added

#### crates/scanner
- UE5 `.uasset` / `.umap` binary parser: header, name table, import table, export table (UE5.1+)
- Rayon-parallel `scan_files()` across all available CPU cores
- Tagged property parser for Blueprint node-level metrics extraction
- Soft object path parser covering `DataTable` row refs, `AnimMontage` slot refs, and `LevelSequence` track refs

#### crates/asset-db
- SQLite asset index with mtime-based differential scan (`filter_changed` runs in O(M+N))
- Multi-filter `find_assets()` supporting type, file-size range, and glob path queries
- Scan history table recording per-scan summary metrics

#### crates/dependency-graph
- petgraph-backed dependency graph with `build()`, `nodes()`, and `in_degree()`
- Cycle detection via Tarjan's SCC algorithm (`find_cycles()`)
- Reverse-dependency impact analysis via BFS with depth tracking (`find_impact()`)

#### crates/dead-asset-detector
- Unreferenced asset enumeration by comparing graph nodes against all known assets

#### crates/redirector-analyzer
- ObjectRedirector asset detection by scanning graph node types

#### crates/bp-analyzer
- Blueprint complexity metrics: node count, EventTick count, cast count, dependency depth
- `is_complex()` threshold checker consumed by the lint engine

#### crates/duplicate-detector
- Same-name duplicate detection across different asset paths
- Approximate texture duplicate detection by file size and asset type

#### crates/lint-engine
- Pluggable `LintRule` trait with `LintViolation` (severity, rule ID, message, path)
- Naming prefix rule with 8 default mappings (T\_, M\_, SM\_, BP\_, SK\_, WBP\_, DA\_, AS\_)
- Texture size rule with configurable per-type byte limit
- Blueprint complexity rule delegating thresholds to `bp-analyzer`

#### crates/material-analyzer
- Texture sample count via export class name scanning
- MaterialInstance chain depth via dependency graph traversal

#### crates/budget-tracker
- Per-asset-type file-size budget enforcement with `BudgetReport` (violation list + summary)

#### crates/watcher
- File-system watcher using the `notify` crate with 300 ms debounce
- Incremental re-scan on `.uasset` / `.umap` change, create, and delete events

#### crates/git-diff
- HEAD-vs-disk asset diff: dependency additions/removals and Blueprint metric deltas
- `compute_diff()` / `diff_asset()` API for use in CI and Watch mode

#### crates/cli — commands
- `scan` — index assets with differential update and stale-detection prompt
- `scan --diff` — report new / removed / changed assets since the previous scan
- `graph` — dependency graph summary with optional `--cycles-only` mode
- `dead-assets` — list unreferenced assets with `--type` filter and human-readable sizes
- `impact` — reverse-dependency walk; `--tree` renders propagation paths
- `redirectors` — detect and list ObjectRedirector assets (exits 1 if any found)
- `find` — multi-filter asset search: `--type`, `--larger-than`, `--smaller-than`, `--path` glob, `--unreferenced`, `--sort-by-size`, `--refs`, `--deps`
- `blueprint` — Blueprint complexity ranking sorted by node count
- `lint` — naming, texture-size, and Blueprint-complexity violations; exits 1 for CI gating
- `budget` — per-type file-size budget report
- `duplicates` — same-name and approximate same-size duplicate groups
- `watch` — real-time change detection with immediate re-analysis
- `check` — single-command CI quality gate (dead assets + cycles + lint)
- `clean` — delete confirmed dead assets from disk with dry-run preview
- `path` — convert between filesystem path and `/Game/` path
- `completions` — generate shell completion scripts (bash, zsh, fish, PowerShell)

#### crates/cli — CI integration features
- `--format json` on all commands for machine-readable output
- `--format github-actions` for inline PR annotations
- `--save-baseline` / `--diff-from` for branch-level CI baseline comparison
- `external_roots` config key to exclude engine/plugin assets from the dependency graph

#### Configuration (`.uasset-lens.toml`)
- `[scan]` section: `exclude_paths` for directory exclusion using prefix matching
- `[lint]` section: per-type naming prefix map and Blueprint complexity thresholds
- `[budget]` section: per-type file-size limits in bytes

### Performance

- Eliminated per-import `String` allocation in `parse_import_entries()`, reducing heap pressure during large project scans ([#216](https://github.com/itsakeyfut/uasset-lens/pull/218))
- Eliminated intermediate `collect()` in `all_edges()`, halving peak memory during graph construction ([#217](https://github.com/itsakeyfut/uasset-lens/pull/219))

### Infrastructure

- GitHub Actions CI running on Ubuntu, Windows, and macOS ([#221](https://github.com/itsakeyfut/uasset-lens/issues/221))
- Release workflow publishing prebuilt binaries (`.zip` / `.tar.gz`) to GitHub Releases on `v*` tag push ([#220](https://github.com/itsakeyfut/uasset-lens/issues/220))
- `docs/ci/` with GitHub Actions example workflows and Git LFS guide

---
