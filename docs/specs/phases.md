# Phase Design

Six-phase structure. **Phase 2 completion is the MVP.**

| Phase | Theme | MVP |
|-------|-------|-----|
| 1 | Foundation: Binary Scanner | — |
| 2 | Core Analysis | **MVP** |
| 3 | CLI Completion | — |
| 4 | Static Analysis | — |
| 5 | Dev Workflow Integration | — |
| 6 | Visualization and Reports | — |

For details, see `docs/roadmap/phase{N}/ROADMAP.md`.

---

## Phase 1 — Foundation: Binary Scanner

### Goal

Parse `.uasset` / `.umap` binaries and index asset metadata and dependencies into SQLite.
Complete when `uasset-lens scan ./Project` works end-to-end.

### Implementation Scope

#### 1. Asset Scanner

- `.uasset` / `.umap` binary parsing (Magic / NameTable / ImportTable / ExportTable)
- Automatic AssetType detection
- Metadata extraction (path / type / size / last_modified / dependencies)
- rayon parallel scanning + mtime delta scanning

#### 2. Asset DB

- SQLite-backed asset index management
- mtime delta scan schema
- Stale record detection and batch upsert

#### 3. CLI: `scan` command

```bash
uasset-lens scan ./Project
uasset-lens scan ./Project --full-scan
```

---

## Phase 2 — Core Analysis ✅ MVP

### Goal

Answer "is it safe to delete this asset?" from the CLI alone.
**MVP achieved** when `graph` / `dead-assets` / `impact` all work.

### Implementation Scope

#### 1. Dependency Graph

- Hard Reference analysis
- Circular dependency detection (Tarjan SCC)
- Impact analysis (direct / transitive separated)

#### 2. Dead Asset Detector

- Detect unreferenced assets (in_degree == 0)

#### 3. CLI: 3 new commands

```bash
uasset-lens graph ./Project
uasset-lens graph ./Project --cycles-only
uasset-lens dead-assets ./Project
uasset-lens dead-assets ./Project --type Texture2D
uasset-lens impact /Game/Characters/BP_Player
```

---

## Phase 3 — CLI Completion

### Goal

Implement all remaining CLI commands and the config file. Polish to OSS-publishable quality.

### Implementation Scope

#### 1. Redirector Analyzer

- Detect and list ObjectRedirector assets

#### 2. Asset Search CLI

- Filtered search by type / size / path / unreferenced flag
- Glob pattern support

#### 3. Config File (`.uasset-lens.toml`)

```toml
[scan]
exclude_paths = ["Content/Dev/", "Content/Test/"]
```

#### 4. CLI: 2 new commands

```bash
uasset-lens redirectors ./Project
uasset-lens find ./Project --type Texture2D --larger-than 4194304
uasset-lens find ./Project --unreferenced --type StaticMesh
uasset-lens find ./Project --path "**/Characters/**"
```

---

## Phase 4 — Static Analysis

### Goal

Expand value from "delete safety" to "Blueprint / asset quality analysis."
`lint` should be usable as a CI quality gate.

### Implementation Scope

#### 1. Blueprint Analyzer

- Node count / branch count / Event Tick / Cast / dependency depth
- Complexity threshold evaluation (called by the Linter)

#### 2. Duplicate Asset Detector

- Same-name asset duplicate detection
- Texture duplicate detection (size + type + name based)

#### 3. Linter

- Naming conventions (T_ / M_ / SM_ / BP_ prefixes, etc.)
- Texture size limit
- Blueprint complexity
- Exit code `1` as a CI quality gate

#### 4. Material Analyzer

- Texture sample count
- MaterialInstance chain depth

#### 5. Performance Budget Tracking

```toml
[budget]
Texture2D.max_size = 4194304    # 4 MB
SoundWave.max_size = 2097152    # 2 MB
```

#### 6. CLI: 4 new commands

```bash
uasset-lens blueprint ./Project
uasset-lens lint ./Project
uasset-lens budget ./Project
uasset-lens duplicates ./Project
```

---

## Phase 5 — Dev Workflow Integration

### Goal

Integrate the tool into the development workflow.
Real-time asset change detection (Watch Mode), Git diff visualization for Blueprint structure,
and GitHub Actions CI integration.

### Implementation Scope

#### 1. Watch Mode

- File system monitoring via the `notify` crate
- Debounce + immediate re-scan on change + issue notification

#### 2. Git Diff Analyzer

- Fetch old version via `git show HEAD:path` for comparison
- Display added/removed dependencies and changed Blueprint metrics

#### 3. CI Integration

- GitHub Actions sample workflow
- `lint` exit code `1` to halt the pipeline

#### 4. CLI: `watch` command

```bash
uasset-lens watch ./Project
```

---

## Phase 6 — Visualization and Reports

### Goal

Visualize CLI analysis results in an egui GUI dashboard and HTML/Markdown reports.
Add Level / Map specific analysis.

### Implementation Scope

#### 1. Level / Map Analyzer

- Actor type counts per Level
- Level dependency graph
- World Partition detection

#### 2. Report Generator

- HTML reports (offline, no CDN required)
- Markdown reports (GitHub Flavored Markdown)

#### 3. GUI Dashboard (egui / eframe)

- Dashboard for scan results
- Unreferenced asset / circular dependency / Blueprint ranking views
- Real-time asset search

#### 4. CLI: `report` command + GUI binary

```bash
uasset-lens report ./Project --format html -o report.html
uasset-lens report ./Project --format markdown
```
