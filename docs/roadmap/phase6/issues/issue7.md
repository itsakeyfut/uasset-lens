# `apps/uasset-lens-desktop` — dashboard: summary panel and dead assets table

## Summary

Implement the dashboard summary panel and the dead assets list in the GUI.
Complete when the dashboard shows correct asset counts and the dead assets table
is sortable by type and size.

## Design Notes

**Summary panel** (displayed at the top of the dashboard):

```
┌─────────────────────────────────────────────────────┐
│  Total Assets: 523   Dead: 12   Cycles: 2   Size: 1.2 GB  │
└─────────────────────────────────────────────────────┘
```

Use `egui` labels in a horizontal layout.

**Dead assets table:**

| Asset Path | Type | Size |
|---|---|---|
| /Game/Textures/T_Old | Texture2D | 2.1 MB |

Use `egui_extras::TableBuilder` for sortable column headers.
Sort state (column + direction) stored in app state.

Clicking a row copies the asset path to the clipboard via `egui::Context::copy_text()`.

## Requirements

- [ ] Add `egui_extras` to `[workspace.dependencies]` (for `TableBuilder`)
- [ ] Implement summary panel showing total assets, dead count, cycle count, total size
- [ ] Implement dead assets table using `egui_extras::TableBuilder`
- [ ] Add sortable columns: Path (alphabetical), Type, Size
- [ ] Clicking a row copies the asset path to clipboard
- [ ] Table updates when `AppState::Ready` data changes (re-scan)

## Related

- Depends on: #6 (app skeleton, AppState::Ready with ProjectData)
- Next: #8 — cycles list and Blueprint ranking
