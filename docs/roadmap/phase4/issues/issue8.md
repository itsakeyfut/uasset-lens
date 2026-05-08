# `crates/lint-engine` — texture size lint rule

## Summary

Implement the texture file size lint rule that flags `Texture2D` assets exceeding
a configurable byte limit.
Complete when a texture above the limit produces a `LintViolation` and one below does not.

## Design Notes

**Rule:**

```rust
pub struct TextureSizeRule {
    pub max_size: u64,   // default: 4 * 1024 * 1024 (4 MB)
}
```

`check()`: if `asset.asset_type == AssetType::Texture2D && asset.file_size > self.max_size`,
emit a `Warning`-severity violation.

```
rule_id: "budget/texture-size"
message: "Texture2D T_Rock exceeds size limit: 5.2 MB > 4.0 MB"
```

## Requirements

- [ ] Implement `TextureSizeRule` struct with `max_size: u64`
- [ ] Implement `TextureSizeRule::default()` with `max_size = 4 * 1024 * 1024`
- [ ] `check()`: no violation for non-Texture2D assets regardless of size
- [ ] `check()`: no violation when `file_size <= max_size`
- [ ] `check()`: violation when `file_size > max_size` with human-readable MB message
- [ ] Unit test: texture at exactly `max_size` → no violation
- [ ] Unit test: texture 1 byte over `max_size` → violation
- [ ] Unit test: StaticMesh over `max_size` → no violation (wrong type)

## Related

- Depends on: #6 (LintRule trait)
- Next: #9 — Blueprint complexity rules
