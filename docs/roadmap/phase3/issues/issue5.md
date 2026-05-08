# `crates/cli` — `find` command

## Summary

Implement the `find` command that queries indexed assets with type, size, path,
and unreferenced filters.
Complete when all filter options work individually and in combination.

## Design Notes

**CLI options:**

```
find <project_dir>
  --type         <AssetType>   filter by asset type (e.g. Texture2D)
  --larger-than  <bytes>       file_size > N
  --smaller-than <bytes>       file_size < N
  --unreferenced               only assets with in_degree == 0
  --path         <pattern>     glob pattern matched against file path
```

**`--unreferenced` flag:**
Build the `DependencyGraph` (`load_graph()`), run `dead_asset_detector::detect()`,
then intersect that result with the `find_assets()` results.

**Text output:**

```
Found 12 assets
===============
/Game/Textures/T_Ground_D        Texture2D    4.2 MB
/Game/Textures/T_Ground_N        Texture2D    2.1 MB
...
```

**JSON output:**

```json
[
  {"asset_path": "/Game/Textures/T_Ground_D", "asset_type": "Texture2D", "file_size": 4404019, "file_path": "..."}
]
```

**Exit codes:** always 0 (zero results is not an error).

## Requirements

- [ ] Implement `find` command handler
- [ ] Build `AssetFilter` from CLI options (`--type`, `--larger-than`, `--smaller-than`, `--path`)
- [ ] Call `db.find_assets(&filter)`
- [ ] When `--unreferenced` is set: intersect `find_assets` results with `dead_asset_detector::detect()` output
- [ ] Implement text output (count header + path/type/size table)
- [ ] Implement JSON output (array of asset objects)
- [ ] Exit code always 0

## Related

- Depends on: Issue #4 (glob in find_assets), Phase 2 Issue #4 (dead-asset-detector for --unreferenced)
- Docs: `docs/specs/cli-design.md` (find output spec)
