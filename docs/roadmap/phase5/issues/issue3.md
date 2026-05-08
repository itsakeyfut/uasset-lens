# `crates/git-diff` — Blueprint asset diff via `git show`

## Summary

Create the `git-diff` crate that compares the current version of a `.uasset` file
against its previous `HEAD` version using `git show`, and produces an `AssetDiff`.
Complete when `diff_asset()` returns a meaningful diff for a changed Blueprint fixture.

## Design Notes

**`git show` execution:**

```rust
let output = std::process::Command::new("git")
    .args(["show", &format!("HEAD:{relative_path}")])
    .current_dir(project_dir)
    .output()?;
```

`relative_path` is the path relative to the git repository root.

If `git show` fails (file not tracked, not a git repo, etc.): return `Ok(None)` — not an error.

**`AssetDiff` struct:**

```rust
pub struct AssetDiff {
    pub asset_path:     AssetPath,
    pub deps_added:     Vec<AssetPath>,
    pub deps_removed:   Vec<AssetPath>,
    pub type_changed:   Option<(AssetType, AssetType)>,   // (old, new)
    pub metrics_delta:  Option<MetricsDelta>,
}

pub struct MetricsDelta {
    pub node_count_delta:       i32,
    pub event_tick_count_delta: i32,
}
```

**Algorithm:**
1. `git show HEAD:<path>` → old binary bytes
2. `std::fs::read(<current_path>)` → new binary bytes
3. `scanner::scan_files([old_tmp_file], content_root)` → old `AssetMetadata`
4. `scanner::scan_files([current_path], content_root)` → new `AssetMetadata`
5. Diff the two `AssetMetadata` values

Write the old bytes to a temp file for scanning.

```rust
pub fn diff_asset(
    asset_path: &AssetPath,
    project_dir: &Path,
    content_root: &Path,
) -> Result<Option<AssetDiff>>
```

## Requirements

- [ ] Create `crates/git-diff` crate
- [ ] Define `AssetDiff` and `MetricsDelta` structs
- [ ] Implement `diff_asset()` using `git show HEAD:<path>` to get old bytes
- [ ] Write old bytes to a `tempfile` for scanning
- [ ] Compute `deps_added` / `deps_removed` by set difference
- [ ] Compute `type_changed` if `old.asset_type != new.asset_type`
- [ ] Compute `metrics_delta` for Blueprint assets
- [ ] Return `Ok(None)` if file is not tracked by git or git is unavailable
- [ ] Add `tempfile` to `[workspace.dependencies]`
- [ ] Unit test: mock old/new `AssetMetadata` → correct `AssetDiff` computed

## Related

- Depends on: Phase 1 Issues #8 (scanner::scan_files), #3 (AssetPath, AssetType)
