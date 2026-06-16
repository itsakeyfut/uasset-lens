# `export` Command — Options

## Synopsis

```
uasset-lens export <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--format <csv|json>`

Output format. **Required** (no default — must be specified explicitly).

`csv` emits RFC 4180 CSV with a header row.
`json` emits a JSON array of objects.

```bash
uasset-lens export ./Project --format csv
uasset-lens export ./Project --format json
```

---

### `--type <TYPE>`

Filter to assets of the specified type only.

```bash
uasset-lens export ./Project --format csv --type Texture2D
uasset-lens export ./Project --format csv --type Blueprint
```

---

### `--larger-than <SIZE>`

Include only assets whose file size exceeds SIZE bytes.

```bash
uasset-lens export ./Project --format csv --larger-than 1048576
```

---

### `--smaller-than <SIZE>`

Include only assets whose file size is less than SIZE bytes.

```bash
uasset-lens export ./Project --format csv --smaller-than 512000
```

---

### `--path <GLOB>`

Include only assets whose game path (`/Game/...`) matches the glob pattern.

```bash
uasset-lens export ./Project --format csv --path "/Game/Characters/**"
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |

Note: `--format` for `export` controls the data format (`csv` or `json`), not the
global text/json display format used by other commands. The global `--format` flag
is shadowed by this command's own `--format`.
