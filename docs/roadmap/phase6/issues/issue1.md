# `crates/level-analyzer` — Level actor counting

## Summary

Create the `level-analyzer` crate and implement Actor type counting from
`.umap` (World) asset metadata.
Complete when `analyze_level()` returns a `LevelMetrics` with a correct Actor count
for a World fixture.

## Design Notes

**`LevelMetrics`:**

```rust
pub struct LevelMetrics {
    pub level_path:         AssetPath,
    pub actor_count:        u32,
    pub actor_type_counts:  HashMap<String, u32>,   // e.g. {"StaticMeshActor": 142, "PointLight": 38}
    pub referenced_assets:  Vec<AssetPath>,          // assets referenced from this level
    pub dependent_levels:   Vec<AssetPath>,          // other World assets in the dependency chain
    pub has_world_partition: bool,
}
```

**Actor counting strategy:**

From the ImportTable, count entries whose `ClassName` maps to Actor-derived classes
(classes ending in `Actor`, `Light`, `Volume`, etc.).
This is an approximation — the full Actor list would require deeper property parsing.

```rust
pub fn analyze_level(
    metadata: &AssetMetadata,
    graph: &DependencyGraph,
) -> Option<LevelMetrics>
```

Returns `None` for non-World assets.

`referenced_assets`: the direct dependencies of this level from `metadata.dependencies`.
`dependent_levels`: filter `referenced_assets` to those with `AssetType::World`.

## Requirements

- [ ] Create `crates/level-analyzer` crate
- [ ] Define `LevelMetrics` struct
- [ ] Implement `analyze_level(metadata, graph) -> Option<LevelMetrics>`
- [ ] Populate `actor_count` and `actor_type_counts` from ImportTable class names
- [ ] Populate `referenced_assets` from `metadata.dependencies`
- [ ] Populate `dependent_levels` by filtering referenced assets to `AssetType::World`
- [ ] Return `None` for non-World assets
- [ ] Unit test: World fixture → `Some(LevelMetrics)` with non-zero `actor_count`
- [ ] Unit test: Texture2D asset → `None`

## Related

- Next: #2 — World Partition detection
- Docs: `docs/roadmap/phase6/ROADMAP.md` — Task 1
