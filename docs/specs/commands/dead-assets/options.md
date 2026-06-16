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

Filter results to a specific asset type.

Accepts the class name as it appears in the UE Import Table (e.g. `Texture2D`,
`Blueprint`, `StaticMesh`, `SkeletalMesh`, `Material`, `SoundWave`).

```bash
uasset-lens dead-assets ./Project --type Texture2D
uasset-lens dead-assets ./Project --type Blueprint
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

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
