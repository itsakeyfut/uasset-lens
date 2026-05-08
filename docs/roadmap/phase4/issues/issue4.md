# `crates/duplicate-detector` — same-name asset detection

## Summary

Create the `duplicate-detector` crate and implement detection of assets that share
the same filename across different content paths.
Complete when `detect_by_name()` groups duplicate filenames correctly.

## Design Notes

**Same-name duplicate:** two or more `AssetRecord`s whose `asset_path` has the same
final path component (the asset name after the last `/`).

Example: `/Game/Characters/T_Rock` and `/Game/Environment/T_Rock` → both have name `T_Rock`.

```rust
pub struct DuplicateGroup {
    pub name:   String,
    pub assets: Vec<AssetPath>,   // 2+ entries
}

pub fn detect_by_name(assets: &[AssetRecord]) -> Vec<DuplicateGroup>
```

Implementation: collect asset name → Vec<AssetPath> in a `HashMap`, then retain only
entries with 2+ paths.

This crate is a pure function layer — no DB, no IO. Takes a `&[AssetRecord]` slice.

## Requirements

- [ ] Create `crates/duplicate-detector` crate
- [ ] Define `DuplicateGroup` struct
- [ ] Implement `detect_by_name(assets: &[AssetRecord]) -> Vec<DuplicateGroup>`
- [ ] Unit test: assets with unique names → empty result
- [ ] Unit test: two assets with the same name in different paths → one group with 2 entries
- [ ] Unit test: three assets with the same name → one group with 3 entries

## Related

- Next: #5 — texture approximate duplicate detection
- Used by: #16 (duplicates command)
