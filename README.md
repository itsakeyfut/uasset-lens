# uasset-lens

**Enforce asset quality gates in CI — without opening the Unreal Editor.**

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

## Features

- **Scan** — index all `.uasset` / `.umap` files into a local SQLite database
- **Dependency graph** — visualize asset dependencies and detect circular references
- **Dead asset detection** — find assets not referenced by anything
- **Impact analysis** — see which assets break if a target asset is renamed or deleted
- **Blueprint complexity** — rank Blueprints by node count and complexity metrics
- **Lint** — enforce naming conventions, texture size budgets, and Blueprint complexity limits
- **Budget** — report assets exceeding per-type size budgets defined in config
- **Duplicates** — find same-name or same-size texture duplicates
- **Redirectors** — list all `ObjectRedirector` assets that should be fixed
- **Watch** — monitor the project for changes and surface new problems in real time

---

## Installation

### Prebuilt binaries (recommended)

Download the latest binary from [GitHub Releases](https://github.com/itsakeyfut/uasset-lens/releases/latest):

| Platform | File |
|---|---|
| Linux x86_64 | `uasset-lens-linux-x86_64.tar.gz` |
| macOS x86_64 | `uasset-lens-macos-x86_64.tar.gz` |
| Windows x86_64 | `uasset-lens-windows-x86_64.zip` |

Extract the archive and place the binary (`uasset-lens` / `uasset-lens.exe`) in a directory on your `PATH`.

### Build from source

Requires Rust 1.85+ (edition 2024):

```bash
git clone https://github.com/itsakeyfut/uasset-lens
cd uasset-lens
cargo install --path apps/uasset-lens-cli
```

---

## Usage

```bash
# Index all assets in your project
uasset-lens scan ./MyProject

# Show dependency graph summary and detect cycles
uasset-lens graph ./MyProject

# List only circular dependencies (exits 1 if any found)
uasset-lens graph --cycles-only ./MyProject

# Find unreferenced assets
uasset-lens dead-assets ./MyProject

# Show what breaks if an asset is deleted
uasset-lens impact ./MyProject/Content/Characters/BP_Player.uasset

# Run all lint rules (exits 1 if violations found)
uasset-lens lint ./MyProject

# Check per-type size budgets
uasset-lens budget ./MyProject

# Find large textures
uasset-lens find ./MyProject --type Texture2D --larger-than 10485760

# Watch for changes in real time (Ctrl+C to stop)
uasset-lens watch ./MyProject

# JSON output (pipe-friendly)
uasset-lens dead-assets ./MyProject --format json | jq '.[] | .path'
```

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
| `1` | Problems detected (violations, cycles, dead assets) |
| `2` | Tool error (IO failure, database not found, parse error) |

`lint` and `graph --cycles-only` exit `1` when problems are found, which automatically
fails a GitHub Actions step and blocks the PR from merging.

---

## Configuration

Create `.uasset-lens.toml` in the project root to configure lint rules and budgets:

```toml
[lint]
blueprint_max_nodes = 150

[budget]
Texture2D = 10485760   # 10 MB per texture
StaticMesh = 52428800  # 50 MB per mesh
```

---

## License

MIT — see [LICENSE](LICENSE).
