# `crates/asset-db` — `find_assets()` glob pattern support

## Summary

Add glob pattern matching to `AssetFilter.path_pattern` so that
`find_assets()` correctly filters assets by path glob (e.g. `"**/Characters/**"`).
Complete when `find_assets()` with a path_pattern returns only matching assets.

## Design Notes

`path_pattern` filtering is done in Rust after the SQL query, not in SQL, to keep query
construction simple and avoid `LIKE` limitations (case sensitivity, special chars).

**Implementation:**

```rust
use globset::{Glob, GlobMatcher};

// Inside find_assets():
let matcher: Option<GlobMatcher> = filter.path_pattern.as_deref()
    .map(|p| Glob::new(p).unwrap().compile_matcher());

rows.into_iter()
    .filter(|r| {
        matcher.as_ref()
            .map(|m| m.is_match(&r.file_path))
            .unwrap_or(true)
    })
    .collect()
```

Match against `file_path` (the filesystem path, e.g. `C:\Project\Content\Characters\BP_Player.uasset`)
rather than `asset_path` (`/Game/Characters/BP_Player`), because `globset` is designed
for filesystem paths and the `**` pattern behaves correctly with path separators.

> **Note**: `globset` should already be in `[workspace.dependencies]` from Phase 1 Issue #11.

## Requirements

- [ ] Compile `path_pattern` into a `globset::GlobMatcher` inside `find_assets()` when set
- [ ] Apply the matcher as a Rust-side post-filter on the SQL result rows
- [ ] Unit test: `"**/Characters/**"` matches assets under a `Characters` directory
- [ ] Unit test: no `path_pattern` returns all assets (unaffected by this change)
- [ ] Unit test: pattern that matches nothing returns empty Vec

## Related

- Depends on: Phase 1 Issue #11 (find_assets base implementation)
- Used by: Issue #5 (find command `--path` option)
