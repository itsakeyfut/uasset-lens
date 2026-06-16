# `duplicates` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                             [asset-db]
2. db.all_assets() → Vec<AssetRecord>                 [asset-db]
3. detect_by_name(&assets)                            [duplicate-detector]
   └── group by asset name (last path segment)
   └── returns groups where count >= 2
4. detect_texture_duplicates(&assets)                 [duplicate-detector]
   └── group by (name, asset_type, file_size) for Texture2D family
   └── returns groups where count >= 2
5. Suppress same-name groups already captured by texture-dup:
   └── texture_dup_names: HashSet<&str>
   └── filter by_name_groups: skip if name in texture_dup_names
6. Merge: texture-dup entries first, then same-name entries
7. Sort by (kind ASC, name ASC) for deterministic output
8. Format and output
9. Return: 1 if any groups found, 0 if all unique
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Name-based detection | `uasset-lens-duplicate-detector` |
| Texture duplicate detection | `uasset-lens-duplicate-detector` |
| Deduplication between detectors | `uasset-lens-cli` |

## Two-Pass Detection Algorithm

Two independent detectors run on the same asset list, then the CLI deduplicates their output:

**Pass 1 — `detect_by_name`**: Groups all assets by their leaf name (e.g. `T_Rock`).
Any name appearing in 2+ different paths is a `same-name` duplicate group.

**Pass 2 — `detect_texture_duplicates`**: Groups Texture2D-family assets by
`(name, type, file_size)`. When the same texture name exists at two paths with
identical sizes, it is almost certainly a true content duplicate — the file was
copied rather than symlinked.

**Deduplication**: If a name appears in the `texture-dup` set, its entry in the
`same-name` set is suppressed. A texture duplicate is always also a same-name
duplicate, but the more specific classification (`texture-dup`) takes precedence.

This prevents a single group from appearing twice in the output.

## Group Key Definitions

| Kind | Key | Meaning |
|---|---|---|
| `texture-dup` | `(name, AssetType, file_size)` | Likely a copied asset (bit-for-bit identical) |
| `same-name` | `name` only | Same leaf name in different paths (may differ in content) |

## Output Sort Order

Groups are sorted by `(kind, name)` lexicographically. `same-name` sorts after
`texture-dup` alphabetically, so texture duplicates always appear first in the output.
