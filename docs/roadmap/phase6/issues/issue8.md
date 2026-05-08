# `apps/uasset-lens-desktop` — dashboard: cycles list and Blueprint complexity ranking

## Summary

Add the circular dependency list and Blueprint complexity ranking panels to the dashboard.
Complete when both panels display correctly populated data from the scan results.

## Design Notes

**Cycles panel:**

Display each cycle as a collapsible `egui::CollapsingHeader`:

```
▶ Cycle 1 (3 assets)
  /Game/Characters/BP_Player
  /Game/Characters/BP_Enemy
  /Game/Characters/BP_Boss
```

Show cycle count in the panel header. If no cycles: show `"No circular dependencies found. ✓"`.

**Blueprint complexity ranking:**

A `TableBuilder` table sorted by `node_count` descending by default:

| Rank | Asset | Nodes | Ticks | Casts |
|---|---|---|---|---|
| 1 | BP_Boss | 412 | 3 | 24 |

Only shown if Blueprint metrics are available in `ProjectData` (populated when Phase 4
features are active). Otherwise show a placeholder: `"Blueprint metrics require a Phase 4 scan."`.

## Requirements

- [ ] Implement cycles panel with `CollapsingHeader` per cycle
- [ ] Show "No circular dependencies found." message when `cycles` is empty
- [ ] Implement Blueprint complexity ranking table sorted by node count
- [ ] Show placeholder message when no Blueprint metrics are available
- [ ] Both panels live in the same dashboard tab or as separate collapsible sections

## Related

- Depends on: #7 (dashboard layout established)
- Next: #9 — asset search
