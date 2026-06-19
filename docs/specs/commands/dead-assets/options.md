# `dead-assets` Command — Options

## Synopsis

```
uasset-lens dead-assets <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--type <TYPE>`

Filter results to one or more asset types. Repeatable — pass `--type` multiple times to keep
assets matching **any** of the listed types (OR-combined). With no `--type`, all types are shown.

Accepts the class name as it appears in the UE Import Table (e.g. `Texture2D`,
`Blueprint`, `StaticMesh`, `SkeletalMesh`, `Material`, `SoundWave`).

```bash
uasset-lens dead-assets ./Project --type Texture2D
uasset-lens dead-assets ./Project --type AnimSequence --type SoundWave
```

---

### `--sort-by-size`

Sort results by file size, largest first (default: alphabetical by path).

```bash
uasset-lens dead-assets ./Project --sort-by-size
```

---

### `--min-size <BYTES>`

Exclude assets smaller than this many bytes from the results.

Useful for ignoring small placeholder assets.

```bash
# Only assets 1 MB or larger
uasset-lens dead-assets ./Project --min-size 1048576

# Only assets 4 MB or larger
uasset-lens dead-assets ./Project --min-size 4194304
```

---

### `--exclude <PATTERN>`

Exclude assets whose path contains the given substring. Repeatable.

```bash
uasset-lens dead-assets ./Project --exclude Dev
uasset-lens dead-assets ./Project --exclude Dev --exclude Plugins --exclude Test
```

---

### `--group <type|dir>`

Aggregate results into groups instead of a flat list.

| Value | Behavior |
|---|---|
| `type` | Group by asset type (Texture2D, Blueprint, etc.) |
| `dir` | Group by top-level directory (up to 3 path segments) |

```bash
uasset-lens dead-assets ./Project --group type
uasset-lens dead-assets ./Project --group dir
```

---

### `--include-all-types`

Include sub-object types that are excluded by default.

By default, `MetaData`, `BillboardComponent`, and similar sub-object types are excluded
because they are not independently deletable assets. This flag includes them.

```bash
uasset-lens dead-assets ./Project --include-all-types
```

---

### `--include-soft-refs`

Count soft object references (`FSoftObjectPath`) as incoming references when
determining whether an asset is dead.

By default, only hard import-table references are considered. With this flag,
assets reachable via soft references are not classified as dead.

Requires soft reference data to be populated (scanner must extract `FSoftObjectPath`
strings — see `docs/specs/analyzers/soft-ref-cycles.md`).

```bash
uasset-lens dead-assets ./Project --include-soft-refs
```

---

### `--revival-preview`

For each dead asset, show which assets would start referencing it if it were
connected into the dependency graph (i.e., a "what if this asset were used" analysis).

This is a planning tool for developers deciding whether to delete or reactivate
a dead asset.

```bash
uasset-lens dead-assets ./Project --revival-preview
```

Output per asset:
```
/Game/Unused/M_LegacyRock (Material, 1.2 MB)
  Revival preview: add as dependency of...
    /Game/Meshes/SM_Rock → would increase impact by 3 assets
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
