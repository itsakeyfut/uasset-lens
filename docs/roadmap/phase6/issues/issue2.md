# `crates/level-analyzer` — Level dependency traversal and World Partition detection

## Summary

Extend `level-analyzer` with Level-to-Level dependency graph traversal and
World Partition detection from Export data.
Complete when a World fixture with World Partition has `has_world_partition == true`
and `dependent_levels` is correctly populated.

## Design Notes

**Level dependency traversal:**

Walk `graph.find_impact(level_path)` but only in the dependency direction (not reverse):
use `graph` edges going **out** from the level to find levels it streams in.
Recursively collect all `AssetType::World` nodes reachable from this level in the
dependency graph.

**World Partition detection:**

World Partition assets appear as `ObjectProperty` exports with class name `WorldPartition`
in the Export table. After Phase 4 Issue #1 (property parser), this can be checked by
scanning the Export object list for any export with `class_name == "WorldPartition"`.

For now (if Phase 4 Issue #1 is not yet complete), use a simpler heuristic:
check if the ImportTable contains a reference to `/Script/Engine.WorldPartition`.

```rust
pub fn has_world_partition(metadata: &AssetMetadata) -> bool
```

## Requirements

- [ ] Implement Level-to-Level dependency traversal in `analyze_level()`
- [ ] Collect all transitively reachable `AssetType::World` nodes as `dependent_levels`
- [ ] Implement `has_world_partition(metadata: &AssetMetadata) -> bool`
- [ ] Check for `/Script/Engine.WorldPartition` in import class names as the detection heuristic
- [ ] Populate `has_world_partition` in `LevelMetrics`
- [ ] Unit test: World fixture with known level dependencies → correct `dependent_levels` count
- [ ] Unit test: fixture with World Partition import → `has_world_partition == true`

## Related

- Depends on: #1 (LevelMetrics, analyze_level base)
