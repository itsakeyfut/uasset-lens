# `dead-assets` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                    [asset-db]
2. load_graph(&db, external_roots)           [dependency-graph]
3. dead_asset_detector::detect(&graph, excluded_types)  [dead-asset-detector]
   └── returns Vec<AssetPath> where in_degree == 0
   └── nodes whose type is in excluded_types are filtered out
4. Build type_map: HashMap<&AssetPath, String> from graph nodes
5. Map dead_paths → Vec<DeadAssetEntry>:
   └── join with DB to get file_size
6. Apply filter pipeline (in order):
   a. --type <TYPE>: retain entries where asset_type == TYPE
   b. --min-size <N>: retain entries where file_size >= N
   c. --exclude <PATTERN,...>: retain entries where path does NOT contain any pattern
7. Optional: --sort-by-size → sort descending by file_size
8. Optional: --group <type|dir> → aggregate into groups (no individual listing)
9. Format and output
10. Return: 1 if any entries remain, 0 if empty
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| In-degree-zero detection | `uasset-lens-dead-asset-detector` |
| Graph traversal | `uasset-lens-dependency-graph` |
| DB file size lookup | `uasset-lens-asset-db` |
| Filter pipeline | `uasset-lens-cli` |

## DEFAULT_EXCLUDED_TYPES

Sub-object types are excluded by default to avoid reporting UE engine-internal assets
as "dead". These types appear as graph nodes but have no meaningful standalone existence.
Examples include `MetaData` and similar synthetic class names.

`--include-all-types` passes an empty exclusion list to `detect()`, exposing these nodes.

## Filter Application Order

The filter pipeline runs client-side after the dead asset list is computed:

```
detect() result
  → type filter (exact string match)
  → min_size filter (>= threshold)
  → exclude_patterns (substring match on game path; OR-combined)
  → sort (optional)
  → group (optional, replaces per-asset listing)
```

## GroupMode Implementation

```rust
enum GroupMode { Type, Dir }
```

- `Type`: groups by `asset_type` string (e.g. `"Texture2D"`, `"Blueprint"`)
- `Dir`: groups by `path_depth_prefix(path)` — first 3 path segments
  (e.g. `/Game/Characters/Enemies/BP_Goblin` → `/Game/Characters/`)

Groups are sorted descending by `total_size_bytes`. The group view replaces
individual asset listing and shows `(count assets, total size)` per group.

## External Roots

`cfg.scan.external_roots` lists game paths (e.g. `["/Game/ThirdPerson"]`) that are
treated as implicit entry points. Assets reachable only from these roots are not
considered dead. This prevents marking starter content as dead on projects that
use the Third Person template.
