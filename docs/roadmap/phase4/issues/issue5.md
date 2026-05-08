# `crates/duplicate-detector` — texture approximate duplicate detection

## Summary

Add `detect_texture_duplicates()` to `duplicate-detector` to find Texture2D assets
that are likely duplicates based on file size + asset type + name similarity.
Complete when identical-size textures with the same name in different paths are grouped.

## Design Notes

**Approximate matching criteria (all three must match):**
1. `asset_type == AssetType::Texture2D`
2. Same `file_size`
3. Same asset name (last path component)

This is an intentionally conservative heuristic — no file content is read. Exact
content matching via `xxhash-rust` is a future enhancement noted in the ROADMAP.

```rust
pub fn detect_texture_duplicates(assets: &[AssetRecord]) -> Vec<DuplicateGroup>
```

Implementation: build a `HashMap<(String, u64), Vec<AssetPath>>` keyed on `(name, file_size)`,
filter to Texture2D assets, then retain groups with 2+ entries.

## Requirements

- [ ] Implement `detect_texture_duplicates(assets: &[AssetRecord]) -> Vec<DuplicateGroup>`
- [ ] Filter input to `AssetType::Texture2D` only
- [ ] Group by `(asset_name, file_size)` tuple
- [ ] Return only groups with 2+ entries
- [ ] Unit test: two Texture2D assets with same name + same size → one group
- [ ] Unit test: same name but different sizes → not grouped
- [ ] Unit test: same size but different names → not grouped
- [ ] Unit test: non-Texture2D assets are excluded even with matching name + size

## Related

- Depends on: #4 (DuplicateGroup struct)
- Used by: #16 (duplicates command)
