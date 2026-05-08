# `crates/lint-engine` — naming prefix lint rules

## Summary

Implement the naming convention lint rules that enforce asset type prefixes
(e.g., `T_` for Texture2D, `BP_` for Blueprint).
Complete when a texture named `Rock_D` produces a `LintViolation` and `T_Rock_D` does not.

## Design Notes

**Default prefix map:**

| AssetType | Expected prefix |
|---|---|
| `Texture2D` | `T_` |
| `Material` | `M_` |
| `StaticMesh` | `SM_` |
| `Blueprint` | `BP_` |
| `SkeletalMesh` | `SK_` |
| `AnimBlueprint` | `ABP_` |
| `UserWidget` | `WBP_` |
| `SoundWave` | `S_` |

**Rule implementation:**

```rust
pub struct NamingPrefixRule {
    pub prefixes: HashMap<AssetType, String>,
}
```

`check()`: extract asset name from the last path component of `asset.asset_path`,
check if it starts with the configured prefix. If not, emit a `Warning`-severity violation.

Assets with type `Unknown` are skipped.

**Config integration (Issue #10 will wire this up):**

The rule will read prefixes from `.uasset-lens.toml [lint]` in Issue #10.
For now, construct with `NamingPrefixRule::default()` using the table above.

## Requirements

- [ ] Implement `NamingPrefixRule` struct implementing `LintRule`
- [ ] Implement `NamingPrefixRule::default()` with the 8-entry prefix map
- [ ] `check()` extracts asset name from `asset_path` last component
- [ ] Emit `Warning`-severity violation when prefix does not match
- [ ] Skip assets with `AssetType::Unknown`
- [ ] Unit test: `T_Rock` with Texture2D → no violation
- [ ] Unit test: `Rock` with Texture2D → violation with rule_id `"naming/prefix"`
- [ ] Unit test: `Unknown` asset type → no violation

## Related

- Depends on: #6 (LintRule trait)
- Next: #8 — texture size rules
