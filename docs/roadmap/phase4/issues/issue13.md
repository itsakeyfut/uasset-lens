# `crates/cli` — `blueprint` command

## Summary

Implement the `blueprint` command that displays a complexity ranking of Blueprint
assets in the project.
Complete when `uasset-lens blueprint ./Project` outputs a ranked table of Blueprint
metrics.

## Design Notes

**Flow:**

```
scan results in DB
→ db.find_assets(&AssetFilter { asset_type: Some(Blueprint/AnimBlueprint/UserWidget), .. })
→ for each: bp_analyzer::analyze(metadata)   [metadata loaded from scan, metrics already in DB]
→ sort by node_count descending
→ output
```

> **Note**: `BlueprintMetrics` are stored in the DB via a separate `blueprint_metrics` table
> or embedded in the asset record. Define storage strategy (separate table vs JSON column)
> before implementing this command.

**Text output:**

```
Blueprint Complexity Report
===========================
Rank  Asset                            Nodes  Ticks  Casts  Depth
   1  /Game/Characters/BP_Boss           412      3     24      4
   2  /Game/Characters/BP_Player         312      1     18      3
...
```

**JSON output:**

```json
[
  {"asset_path": "/Game/Characters/BP_Boss", "node_count": 412, "event_tick_count": 3, "cast_count": 24, "dependency_depth": 4}
]
```

## Requirements

- [ ] Decide and implement storage of `BlueprintMetrics` in the DB (separate table or JSON column in `assets`)
- [ ] Implement `blueprint` command handler
- [ ] Query Blueprint-type assets and their metrics from the DB
- [ ] Sort results by `node_count` descending
- [ ] Implement text output table (rank / path / node count / tick count / cast count / depth)
- [ ] Implement JSON output (array of metric objects)

## Related

- Depends on: #3 (bp-analyzer), #2 (BlueprintMetrics in scan)
