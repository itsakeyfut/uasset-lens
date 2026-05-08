# `apps/uasset-lens-desktop` — asset search with real-time filtering

## Summary

Add a real-time asset search panel to the GUI that filters the full asset list
by name, type, and size as the user types.
Complete when typing in the search box instantly filters the asset table.

## Design Notes

**Search bar:**

```
[ 🔍 Search assets...        ] [Type ▼] [> 1 MB ▼]
```

- Text input: `egui::TextEdit::singleline` for name/path substring match
- Type dropdown: `egui::ComboBox` with `AssetType` variants + "All"
- Size filter: `egui::ComboBox` with preset options (Any / >1MB / >4MB / >10MB)

**Filtering logic:**

Applied as a Rust-side filter on `ProjectData.assets` each frame:

```rust
let filtered: Vec<&AssetRecord> = assets.iter()
    .filter(|r| query.is_empty() || r.asset_path.as_str().contains(&query))
    .filter(|r| type_filter.is_none() || r.asset_type == type_filter)
    .filter(|r| size_filter == 0 || r.file_size > size_filter)
    .collect();
```

No debounce needed — `egui` only redraws when there is input, so filtering on every
frame is fine for ≤100k assets.

**Results table:** same `TableBuilder` layout as dead assets table. Display count above table.

## Requirements

- [ ] Implement search panel with text input, type dropdown, and size dropdown
- [ ] Apply all three filters simultaneously on each frame
- [ ] Show filtered result count above the table (e.g. "Showing 12 of 523 assets")
- [ ] Results table uses `TableBuilder` with Path / Type / Size columns
- [ ] Clearing all filters restores the full asset list

## Related

- Depends on: #7 (TableBuilder pattern established)
- Next: #10 — dependency view
