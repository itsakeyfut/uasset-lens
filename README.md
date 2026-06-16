# uasset-lens

**Enforce asset quality gates in CI — without opening the Unreal Editor.**

[![CI](https://github.com/itsakeyfut/uasset-lens/actions/workflows/ci.yml/badge.svg)](https://github.com/itsakeyfut/uasset-lens/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

uasset-lens is a fast, CLI-first static analyzer for Unreal Engine 5 `.uasset` / `.umap`
files. It reads the binary format directly, so it runs in seconds on any CI server with no
editor process, no project compile, and no GPU required.

---

## Why uasset-lens?

Unreal Engine's built-in tools (Reference Viewer, Size Map, Data Validation) are powerful,
but they share a fundamental limitation: **they require the editor to be running**. This
makes them unsuitable for CI pipelines, PR automation, and long-term trend tracking.

uasset-lens fills three gaps that UE leaves open:

### 1. Quality tracking over time

UE shows you the current state. It cannot tell you that Blueprint complexity increased in
today's PR, or that total texture size has grown 20% over the past month. uasset-lens
gives assets the same kind of static-analysis feedback loop that linters give code.

### 2. Editor-free, CI-native execution

A large UE project takes 2–5 minutes to open in the editor. uasset-lens scans 1,000
assets in under 5 seconds by parsing `.uasset` binaries directly. It runs on any machine
with no GPU, no project build, and no editor license — including GitHub Actions runners.

### 3. Soft-reference visibility

UE's Reference Viewer only follows hard references. Assets loaded via `DataTable` rows,
`AnimMontage` slots, or `TSoftObjectPtr` fields appear as unreferenced, causing false
positives in dead-asset detection and incomplete dependency graphs. uasset-lens tracks
these soft references explicitly.

---

## System requirements

| Requirement | Details |
|---|---|
| Unreal Engine | **5.1 or later** (UE4 is not supported) |
| Operating system | Windows, macOS, Linux (x86_64) |
| Asset formats | `.uasset` and `.umap` files |
| **Not supported** | IoStore containers (`.pak`, `.utoc`, `.ucas`) |

> **Note:** IoStore is a UE5 package format used for cooked/shipped builds. uasset-lens
> analyzes source-control assets in your project's `Content/` folder, not cooked output.

---

## Installation

### Prebuilt binaries (recommended)

Download the latest binary from [GitHub Releases](https://github.com/itsakeyfut/uasset-lens/releases/latest):

| Platform | File |
|---|---|
| Linux x86_64 | `uasset-lens-linux-x86_64.tar.gz` |
| macOS x86_64 | `uasset-lens-macos-x86_64.tar.gz` |
| Windows x86_64 | `uasset-lens-windows-x86_64.zip` |

Extract the archive and place `uasset-lens` (or `uasset-lens.exe`) somewhere on your `PATH`.

### Build from source

Requires Rust 1.96+:

```bash
git clone https://github.com/itsakeyfut/uasset-lens
cd uasset-lens
cargo install --path apps/uasset-lens-cli
```

---

## Quick start

```bash
# 1. Index all assets into a local database (~5 s for 1,000 assets)
uasset-lens scan ./MyProject

# 2. Find assets that nothing references — candidates for deletion
uasset-lens dead-assets ./MyProject

# 3. See what breaks if you rename or delete one asset
uasset-lens impact ./MyProject/Content/Characters/BP_Player.uasset

# 4. Run all health checks at once (exits 1 if anything is wrong)
uasset-lens check ./MyProject

# 5. Output JSON for scripting
uasset-lens dead-assets ./MyProject --format json | jq '.[] | .path'
```

The database is written to `<project_dir>/.uasset-lens/uasset-lens.db` automatically.
Add that path to `.gitignore`.

---

## Command reference

All commands share three global flags:

| Flag | Description |
|---|---|
| `--format <text\|json\|github-actions>` | Output format (default: `text`) |
| `--db <path>` | Override the database file location |
| `-y` / `--yes` | Skip confirmation prompts (for CI) |

---

### `scan`

Index all `.uasset` / `.umap` files in a project directory into the local database.

```
uasset-lens scan <project_dir> [options]
```

| Option | Description |
|---|---|
| `--full-scan` | Re-scan all files regardless of modification time |
| `--diff` | Show a diff of changes compared to the previous scan |
| `--save-baseline <name>` | Save the scan result as a named baseline |
| `--diff-from <name>` | Diff against a named baseline (implies `--diff`) |

**Output:**
```
Scanning ./MyProject/Content... (1000 files)
  + 3 new assets indexed
  ~ 5 assets updated (mtime changed)
  ? 2 assets removed from disk

The following DB records have no corresponding file on disk:
  /Game/Old/BP_Deprecated.uasset
  /Game/Temp/M_Test.uasset
Remove these records from DB? [y/N]: y

✓ 998 assets total, 2 records cleaned, 2 skipped (parse error)

Skipped:
  WARN Content/Broken/BP_X.uasset: invalid magic number
  WARN Content/Old/M_Y.uasset: unsupported version
```

---

### `graph`

Show the dependency graph summary and detect circular dependencies.

```
uasset-lens graph <project_dir> [options]
```

| Option | Description |
|---|---|
| `--cycles-only` | Show only circular dependencies (exits 1 if any found) |

**Output:**
```
Dependency Graph Summary
  Total assets   : 998
  Total edges    : 4,231
  Circular deps  : 2 cycles detected

Cycles:
  [1] BP_Player → BP_Enemy → BP_GameMode → BP_Player
  [2] M_Rock → MF_Shared → M_Rock
```

---

### `dead-assets`

List assets that are not referenced by any other asset.

```
uasset-lens dead-assets <project_dir> [options]
```

| Option | Description |
|---|---|
| `--type <AssetType>` | Filter by asset type (e.g. `Texture2D`, `Blueprint`) |
| `--sort-by-size` | Sort results by file size, largest first |
| `--min-size <bytes>` | Exclude assets smaller than this size |
| `--exclude <pattern>` | Exclude paths containing this substring (repeatable) |
| `--group <type\|dir>` | Aggregate results by asset type or top-level directory |

**Output:**
```
/Game/Unused/T_OldTexture      (Texture2D, 2.1 MB)
/Game/Characters/SK_OldEnemy   (SkeletalMesh, 8.4 MB)

Dead Assets (47 found)
```

Exits `1` when dead assets are found, `0` when none.

---

### `deps`

Show the forward dependency tree of an asset — everything it depends on.

```
uasset-lens deps <asset_path> [options]
```

| Option | Description |
|---|---|
| `--depth <n>` | Maximum recursion depth (default: unlimited) |
| `--size-only` | Print only the summary line, not the full tree |

**Output:**
```
/Game/Characters/BP_Player
  /Game/Characters/SK_Player            (SkeletalMesh, 12.3 MB)
  /Game/Materials/M_Player              (Material, 1.2 MB)
    /Game/Textures/T_Player_D           (Texture2D, 4.0 MB)
    /Game/Textures/T_Player_N           (Texture2D, 2.0 MB)

4 dependencies, 19.5 MB total
```

---

### `impact`

Show which assets would break if the target asset were deleted or renamed.

```
uasset-lens impact <asset_path> [options]
```

| Option | Description |
|---|---|
| `--tree` | Show the full propagation tree instead of flat lists |

**Output:**
```
Impact Analysis: /Game/Characters/BP_Player

Direct referencing (3):
  /Game/Levels/L_Main.umap
  /Game/UI/WBP_HUD.uasset
  /Game/GameModes/BP_GameMode.uasset

Transitive referencing (12):
  /Game/Levels/L_Tutorial.umap
  ... (9 more)

Total impact: 12 assets
```

Exits `1` when the impact set is non-empty, `0` when nothing references the target.

---

### `redirectors`

List all `ObjectRedirector` assets in the project. These are left behind when assets are
moved or renamed inside the editor; accumulating them bloats packages and cook times.

```
uasset-lens redirectors <project_dir>
```

**Output:**
```
ObjectRedirectors (5 found)
===========================
/Game/Characters/OldName
/Game/Meshes/SM_OldRock
/Game/Materials/M_Deprecated
/Game/UI/WBP_OldWidget
/Game/Blueprints/BP_OldEnemy

Note: redirect target resolution is available in Phase 4 analysis.
```

Exits `1` when redirectors are found, `0` when none.

---

### `find`

Search and filter assets by type, size, or path pattern.

```
uasset-lens find <project_dir> [options]
```

| Option | Description |
|---|---|
| `--type <AssetType>` | Filter by asset type (e.g. `Texture2D`, `Blueprint`) |
| `--larger-than <bytes>` | Minimum file size |
| `--smaller-than <bytes>` | Maximum file size |
| `--unreferenced` | Show only assets not referenced by anything |
| `--path <pattern>` | Filter by glob path pattern (e.g. `"**/Characters/**"`) |
| `--sort-by-size` | Sort results by file size, largest first |
| `--refs <game_path>` | Show only assets that reference this game path |
| `--deps <game_path>` | Show only assets that this game path directly depends on |

**Examples:**
```bash
uasset-lens find ./Project --type Texture2D --larger-than 4194304
uasset-lens find ./Project --unreferenced --type StaticMesh
uasset-lens find ./Project --path "**/Characters/**"
```

**Output:**
```
/Game/Textures/T_Rock_D   (Texture2D, 4.0 MB)
/Game/Textures/T_Rock_N   (Texture2D, 2.0 MB)

2 assets found
```

---

### `blueprint`

Show a complexity ranking of Blueprint assets by node count and other metrics.

```
uasset-lens blueprint <project_dir>
```

**Output:**
```
Blueprint Complexity Report
===========================
Rank  Asset                                       Nodes  Ticks  Casts  Depth
   1  /Game/Player/BP_PlayerController               842     31      5      7
   2  /Game/AI/BP_EnemyAI                            631     18      3      5
   3  /Game/UI/WBP_MainMenu                          412      7      1      2

  3 Blueprint asset(s) ranked
```

---

### `stats`

Show a size and composition overview of the project.

```
uasset-lens stats <project_dir> [options]
```

| Option | Description |
|---|---|
| `--top <n>` | Number of folders and largest assets to show (default: 5 folders, 10 assets) |

**Output:**
```
Project Stats: ./MyProject
  Total assets    :   998
  Total size      : 1.2 GB
  Asset types     :    12

Top folders by size:
  /Game/Characters      342 MB   (28%)
  /Game/Environments    289 MB   (24%)

Largest assets:
  /Game/Environments/SM_LargeRock   (StaticMesh, 52.4 MB)
```

---

### `budget`

Report assets exceeding the per-type size budgets defined in `.uasset-lens.toml`.

```
uasset-lens budget <project_dir>
```

**Output:**
```
Budget Report

  OVER  /Game/Textures/T_Landscape_D   Texture2D   12.4 MB  (limit 10.0 MB, +24%)
  OVER  /Game/Meshes/SM_Building       StaticMesh  67.1 MB  (limit 50.0 MB, +34%)

2 assets over budget
```

Exits `1` when any asset exceeds its budget, `0` when all assets are within limits.

---

### `duplicates`

Find same-name or same-size texture asset groups that are likely redundant.

```
uasset-lens duplicates <project_dir>
```

**Output:**
```
Duplicate Groups (3 found)

  [same-name] T_Rock_D
    /Game/Environments/T_Rock_D    (Texture2D, 4.0 MB)
    /Game/Characters/T_Rock_D      (Texture2D, 4.0 MB)
```

---

### `lint`

Run all lint rules and report violations. Exits `1` if any violations are found.

Rules include: naming prefix conventions, Blueprint node count limits, texture budget
thresholds, and more. Configure rules in `.uasset-lens.toml`.

```
uasset-lens lint <project_dir>
```

**Output:**
```
Lint Results

  ERROR  /Game/Blueprints/gamemode      Blueprint name should start with 'BP_'
  WARN   /Game/Textures/Rock_Diffuse    Texture name should start with 'T_'
  WARN   /Game/Blueprints/BP_Player     Node count 312 exceeds limit 150

3 violations (1 error, 2 warnings)
```

---

### `check`

Run all (or selected) health checks at once. Exits `1` if any check finds problems.

```
uasset-lens check <project_dir> [options]
```

| Option | Description |
|---|---|
| `--only <list>` | Run only these checks (comma-separated) |
| `--skip <list>` | Skip these checks (comma-separated) |

Available checks: `dead-assets`, `cycles`, `redirectors`, `lint`, `budget`, `duplicates`

**Examples:**
```bash
uasset-lens check ./Project                            # run all checks
uasset-lens check ./Project --only cycles,lint         # only cycles and lint
uasset-lens check ./Project --skip dead-assets         # skip dead-asset detection
```

---

### `clean`

Delete confirmed dead assets from disk. Prompts for confirmation unless `-y` is passed.

```
uasset-lens clean <project_dir> [options]
```

| Option | Description |
|---|---|
| `--dry-run` | List deletion targets without deleting; exits 0 |
| `--min-size <bytes>` | Exclude assets smaller than this size |
| `--exclude <pattern>` | Exclude paths containing this substring (repeatable) |
| `--path <pattern>` | Filter by glob path pattern |

---

### `watch`

Monitor the project directory for file changes and print new problems as assets are
modified. Press Ctrl+C to stop.

```
uasset-lens watch <project_dir>
```

---

### `path`

Convert between filesystem paths and UE game paths (`/Game/...`).

```
uasset-lens path <input> [options]
```

| Option | Description |
|---|---|
| `--to-file` | Convert a game path to a filesystem path |
| `--content-root <dir>` | Content root directory (auto-detected if omitted) |

**Examples:**
```bash
uasset-lens path ./MyProject/Content/Characters/BP_Player.uasset
# → /Game/Characters/BP_Player

uasset-lens path /Game/Characters/BP_Player --to-file
# → ./MyProject/Content/Characters/BP_Player.uasset
```

---

### `completions`

Generate shell completion scripts.

```
uasset-lens completions <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`

**Setup:**
```bash
# bash
uasset-lens completions bash >> ~/.bash_completion

# zsh
uasset-lens completions zsh > ~/.zfunc/_uasset-lens

# fish
uasset-lens completions fish > ~/.config/fish/completions/uasset-lens.fish
```

---

## Configuration

Create `.uasset-lens.toml` in the project root and commit it to version control so the
whole team shares the same settings.

```toml
# .uasset-lens.toml

[scan]
# Paths to exclude during scan (relative to project root, prefix-matched)
exclude_paths = [
    "Content/Dev/",
    "Content/Test/",
    "Content/Developers/",
]
# Game path prefixes treated as external (never flagged as dead)
external_roots = ["/Engine/", "/Script/"]

[lint]
blueprint_max_nodes      = 150
blueprint_max_event_tick = 3
blueprint_max_cast_count = 20

# Asset naming prefix rules: type → required prefix
[lint.naming_prefix]
Blueprint   = "BP_"
Texture2D   = "T_"
StaticMesh  = "SM_"
Material    = "M_"

[budget]
# Per-type maximum file size in bytes
Texture2D.max_size  = 10485760   # 10 MB
StaticMesh.max_size = 52428800   # 50 MB
SoundWave.max_size  = 5242880    #  5 MB

[diff]
# Warn when an asset grows by more than this percentage between scans
size_increase_threshold_pct = 10
```

All sections are optional — missing sections use built-in defaults.

---

## CI Integration

Integrate `uasset-lens` into your CI pipeline to block PRs that introduce circular
dependencies or lint violations.

**Quick start** — copy [`docs/ci/github-actions.yml`](docs/ci/github-actions.yml) into
`.github/workflows/` and replace `./YourProject` with your project directory name.

**Asset storage in Git** — see [`docs/ci/git-lfs-guide.md`](docs/ci/git-lfs-guide.md)
for guidance on choosing between direct commit and Git LFS.

### Exit codes

| Exit code | Meaning |
|-----------|---------|
| `0` | Success — no problems found |
| `1` | Problems detected (violations, cycles, dead assets, etc.) |
| `2` | Tool error (IO failure, database not found, parse error) |

`lint`, `graph --cycles-only`, `budget`, and `check` exit `1` when problems are found,
which automatically fails a GitHub Actions step and blocks the PR from merging.

### GitHub Actions annotations

Pass `--format github-actions` to emit inline PR annotations for lint and budget violations:

```yaml
- name: Lint assets
  run: uasset-lens lint ./MyProject --format github-actions
```

---

## Contributing

Bug reports, feature requests, and questions are welcome — please
[open an issue](https://github.com/itsakeyfut/uasset-lens/issues).

When filing a bug, include the output of `uasset-lens --version` and the relevant
command that failed.

---

## License

MIT — see [LICENSE](LICENSE).
