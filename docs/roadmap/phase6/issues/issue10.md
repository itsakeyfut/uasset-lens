# `apps/uasset-lens-desktop` — dependency view

## Summary

Add a dependency view panel that shows the direct and transitive impact of a selected
asset, equivalent to the `uasset-lens impact` CLI command output.
Complete when selecting an asset from the search panel populates the dependency view
with its referencing assets.

## Design Notes

**Trigger:** clicking an asset path in any table (dead assets, search results) opens
the dependency view for that asset.

**Panel layout:**

```
Impact: /Game/Characters/BP_Player
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Direct impact (1 hop)     3 assets
  /Game/Maps/L_MainLevel
  /Game/UI/WBP_HUD
  /Game/Characters/BP_Boss

Transitive impact (2+ hops)  7 assets
  /Game/Maps/L_OutdoorArea
  ...

Total: 10 assets affected
```

Use `graph.find_impact(selected_path)` from the `DependencyGraph` stored in `ProjectData`.
If the asset is not in the graph, show: `"Asset not found in dependency graph."`.

**Navigation:** clicking any path in the dependency view navigates to that asset's
own dependency view (so the user can explore the graph).

**Back button:** keep a history stack (`Vec<AssetPath>`) so the user can navigate back.

## Requirements

- [ ] Implement dependency view panel showing `ImpactResult.direct` and `ImpactResult.transitive`
- [ ] Trigger panel from asset clicks in dead assets table and search results
- [ ] Display "not found" message when selected asset is absent from graph
- [ ] Make each path in the panel clickable to navigate to that asset's dependency view
- [ ] Implement navigation history (back button / stack)
- [ ] Show total affected count below both lists

## Related

- Depends on: #9 (asset selection from search), Phase 2 Issue #3 (find_impact, ImpactResult)
- Closes Phase 6
