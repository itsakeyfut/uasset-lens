# `crates/budget-tracker` — asset budget checking

## Summary

Create the `budget-tracker` crate that checks asset file sizes against per-type
budgets defined in `.uasset-lens.toml`.
Complete when assets exceeding their budget are listed in `BudgetReport`.

## Design Notes

**TOML schema extension (`[budget]` section):**

```toml
[budget]
Texture2D.max_size = 4194304    # 4 MB
SoundWave.max_size = 2097152    # 2 MB
StaticMesh.max_size = 10485760  # 10 MB
```

**Extend `ConfigFile`:**

```rust
#[derive(Default, serde::Deserialize)]
pub struct BudgetConfig {
    #[serde(flatten)]
    pub limits: HashMap<String, AssetBudget>,
}

#[derive(serde::Deserialize)]
pub struct AssetBudget {
    pub max_size: u64,
}
```

**`BudgetReport`:**

```rust
pub struct BudgetReport {
    pub violations: Vec<BudgetViolation>,
}

pub struct BudgetViolation {
    pub asset_path: AssetPath,
    pub asset_type: AssetType,
    pub file_size:  u64,
    pub max_size:   u64,
}
```

**`check_budget()`:**

```rust
pub fn check_budget(assets: &[AssetRecord], config: &BudgetConfig) -> BudgetReport
```

For each asset, look up `config.limits` by `asset_type.to_string()`. If the asset's
`file_size > max_size`, add a `BudgetViolation`.

## Requirements

- [ ] Create `crates/budget-tracker` crate
- [ ] Define `BudgetReport` and `BudgetViolation` structs
- [ ] Implement `check_budget(assets, config) -> BudgetReport`
- [ ] Extend `ConfigFile` with `BudgetConfig` and `[budget]` TOML deserialization
- [ ] Unit test: asset within budget → no violation
- [ ] Unit test: asset 1 byte over budget → violation
- [ ] Unit test: asset type with no budget configured → no violation
- [ ] Unit test: multiple asset types each with separate budgets

## Related

- Depends on: Phase 3 Issue #2 (ConfigFile)
- Used by: #15 (budget command)
