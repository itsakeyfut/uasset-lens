# `find` Command — Options

## Synopsis

```
uasset-lens find <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--type <TYPE>`

Filter by asset type. Accepts the class name as it appears in the UE Import Table.

```bash
uasset-lens find ./Project --type Texture2D
uasset-lens find ./Project --type Blueprint
uasset-lens find ./Project --type StaticMesh
```

---

### `--larger-than <BYTES>`

Minimum file size in bytes. Only assets larger than this value are returned.

```bash
uasset-lens find ./Project --larger-than 4194304   # > 4 MB
```

---

### `--smaller-than <BYTES>`

Maximum file size in bytes. Only assets smaller than this value are returned.

```bash
uasset-lens find ./Project --smaller-than 1048576  # < 1 MB
```

---

### `--unreferenced`

Show only assets with no incoming references (in-degree zero in the dependency graph).

```bash
uasset-lens find ./Project --unreferenced
uasset-lens find ./Project --unreferenced --type Texture2D
```

---

### `--path <PATTERN>`

Filter by glob path pattern. The pattern is matched against the UE game path
(`/Game/...`).

```bash
uasset-lens find ./Project --path "**/Characters/**"
uasset-lens find ./Project --path "**/Plugins/**"
uasset-lens find ./Project --path "/Game/UI/**"
```

---

### `--sort-by-size`

Sort results by file size, largest first (default: alphabetical by path).

```bash
uasset-lens find ./Project --type Texture2D --sort-by-size
```

---

### `--refs <GAME_PATH>`

Show only assets that reference the given game path (direct or transitive).

Accepts game paths only (`/Game/...` format).

```bash
uasset-lens find ./Project --refs /Game/Materials/M_Rock
```

---

### `--deps <GAME_PATH>`

Show only assets that the given game path directly depends on.

Accepts game paths only (`/Game/...` format).

```bash
uasset-lens find ./Project --deps /Game/Characters/BP_Player
```

---

### `--has-violation <RULE>`

Filter to assets that have at least one active violation matching the given rule ID.

Accepts a full rule ID or a category prefix.

```bash
uasset-lens find ./Project --has-violation lint/naming/blueprint-prefix
uasset-lens find ./Project --has-violation budget/texture2d
uasset-lens find ./Project --has-violation lint/blueprint
```

Requires violation data to be present. If `check` has never been run, returns 0 results with a warning.

---

### `--sort <FIELD>`

Sort results by the given field (default: `path`).

| Value | Sort order |
|---|---|
| `path` | Alphabetical by game path (default) |
| `size` | File size descending |
| `type` | Asset type alphabetical, then path |
| `health` | Health score ascending (worst first) |

```bash
uasset-lens find ./Project --type Texture2D --sort size
uasset-lens find ./Project --has-violation lint --sort health
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
