# `export` Command — Specification

## Purpose

Export asset data from the DB to CSV or JSON for use in external tools such as
spreadsheets, BI dashboards, or custom scripts.

```bash
uasset-lens export ./Project --format csv > assets.csv
uasset-lens export ./Project --format json > assets.json
uasset-lens export ./Project --format csv --type Texture2D
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Export completed successfully |
| `2` | Execution error (DB not found, I/O failure) |

---

## Output Columns

| Column | Type | Description |
|---|---|---|
| `path` | string | Game path (`/Game/...`) |
| `type` | string | Asset type string (e.g. `Blueprint`, `Texture2D`) |
| `file_size` | integer | File size in bytes |
| `deps_count` | integer | Number of direct outgoing dependencies |
| `in_degree` | integer | Number of assets that directly reference this asset |

---

## CSV Output

```csv
path,type,file_size,deps_count,in_degree
/Game/Characters/BP_Player,Blueprint,1468006,3,12
/Game/Characters/SK_Player,SkeletalMesh,8808038,0,3
/Game/Textures/T_Rock_D,Texture2D,2097152,0,7
```

- Header row always present.
- Fields are not quoted unless the value contains a comma, double-quote, or newline.
- `path` values never contain commas, so quoting is only required for edge cases in
  asset type strings (none expected in practice).
- Encoding: UTF-8 without BOM.

---

## JSON Output

Array of objects with the same five fields:

```json
[
  {
    "path": "/Game/Characters/BP_Player",
    "type": "Blueprint",
    "file_size": 1468006,
    "deps_count": 3,
    "in_degree": 12
  },
  {
    "path": "/Game/Characters/SK_Player",
    "type": "SkeletalMesh",
    "file_size": 8808038,
    "deps_count": 0,
    "in_degree": 3
  }
]
```

---

## Filters

Filters narrow the set of assets included in the export. Multiple filters are
ANDed together.

| Filter | Description |
|---|---|
| `--type <TYPE>` | Include only assets of the given type |
| `--larger-than <SIZE>` | Include only assets larger than SIZE bytes |
| `--smaller-than <SIZE>` | Include only assets smaller than SIZE bytes |
| `--path <GLOB>` | Include only assets whose game path matches the glob pattern |

Filter semantics are identical to those of the `find` command.

---

## Output Ordering

Assets are sorted by `path` ascending (lexicographic). This ensures stable, diff-able
output across runs.

---

## Error Cases

| Condition | Behaviour |
|---|---|
| DB not found | Exit `2`: `error: database not found — run 'scan' first` |
| No assets match filters | Emit header row only (CSV) or empty array (JSON); exit 0 |
