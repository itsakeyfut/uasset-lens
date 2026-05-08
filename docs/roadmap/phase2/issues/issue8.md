# `crates/cli` — `impact` command

## Summary

Implement the `impact` command that shows which assets would break if the target
asset were deleted, split into direct and transitive referencing assets.
Complete when `uasset-lens impact /Game/Characters/BP_Player` and filesystem path
variants both produce correct output.

## Design Notes

**Argument:** `<asset_path>` — accepts either:
- A game path: `/Game/Characters/BP_Player`
- A filesystem path: `./Project/Content/Characters/BP_Player.uasset`
  → convert via `AssetPath::from_fs_path(content_root, file_path)`

If the asset is not found in the graph after conversion: print an error to stderr and exit 2.

**Text output (from `docs/specs/cli-design.md`):**

```
Impact Analysis: /Game/Characters/BP_Player
===========================================
Direct impact   (1 hop):  3 assets
  /Game/Maps/L_MainLevel
  /Game/UI/WBP_HUD
  /Game/Characters/BP_Boss

Transitive impact (2+ hops):  7 assets
  /Game/Maps/L_OutdoorArea
  ...

Total: 10 assets affected.
```

**JSON output:**

```json
{
  "target":     "/Game/Characters/BP_Player",
  "direct":     ["/Game/Maps/L_MainLevel", ...],
  "transitive": ["/Game/Maps/L_OutdoorArea", ...],
  "total":      10
}
```

**Exit codes:** impact found (total > 0) → 1; no impact → 0; execution error → 2.

## Requirements

- [ ] Implement `impact` command handler accepting a game path or filesystem path argument
- [ ] Convert filesystem path to `AssetPath` using `AssetPath::from_fs_path()` when extension is present
- [ ] Return exit 2 with error message if target not found in graph
- [ ] Call `graph.find_impact(target)` to get `ImpactResult`
- [ ] Implement text output with direct/transitive sections and total
- [ ] Implement JSON output matching the spec
- [ ] Exit code 1 when total > 0, 0 when no impact found

## Related

- Depends on: Phase 2 Issue #3 (find_impact, ImpactResult), Phase 2 Issue #6 (load_graph)
- Closes Phase 2 (MVP achieved)
- Docs: `docs/specs/cli-design.md` (impact output spec)
