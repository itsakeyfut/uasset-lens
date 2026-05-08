# `crates/cli` — `dead-assets` command

## Summary

Implement the `dead-assets` command that lists unreferenced assets by combining
`DependencyGraph` analysis with DB asset records.
Complete when `uasset-lens dead-assets ./Project` lists unreferenced assets and
`--type Texture2D` filters the output correctly.

## Design Notes

**Flow:**

```
load_graph(db)
  └─ dead_asset_detector::detect(&graph) → Vec<AssetPath>
  └─ optional: filter by --type (look up AssetType from graph.nodes())
  └─ fetch file_size from db.get_asset() for each dead path (for display)
  └─ output
```

**Text output (from `docs/specs/cli-design.md`):**

```
Dead Assets (5 found)
=====================
/Game/Textures/T_Old        Texture2D    2.1 MB
/Game/Meshes/SM_Unused      StaticMesh   512 KB
...
```

**`--type <AssetType>` flag:** filter dead assets to only the specified type.
Match `AssetType` from `AssetNode.asset_type` in the graph.

**JSON output:**

```json
[
  {"asset_path": "/Game/Textures/T_Old", "asset_type": "Texture2D", "file_size": 2202009}
]
```

**Exit codes:** dead assets found → 1; none found → 0; execution error → 2.

## Requirements

- [ ] Implement `dead-assets` command handler
- [ ] Call `dead_asset_detector::detect(&graph)` to get unreferenced paths
- [ ] Apply optional `--type` filter using `AssetNode.asset_type` from the graph
- [ ] Fetch `file_size` from `db.get_asset()` for each dead asset for display
- [ ] Implement text output with count header + table (path / type / size)
- [ ] Implement JSON output (array of asset objects)
- [ ] Exit code 1 when dead assets found, 0 when clean

## Related

- Depends on: Phase 2 Issue #4 (dead-asset-detector), Phase 2 Issue #6 (load_graph)
- Next: #8 — impact command
- Docs: `docs/specs/cli-design.md` (dead-assets output spec)
