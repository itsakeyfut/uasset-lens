# `crates/cli` — `budget` command

## Summary

Implement the `budget` command that reports assets exceeding their configured
per-type size budgets.
Complete when `uasset-lens budget ./Project` lists over-budget assets and a per-type summary.

## Design Notes

**Flow:**

```
load_config(project_dir) → config.budget
→ db.all_assets()
→ budget_tracker::check_budget(&assets, &config.budget) → BudgetReport
→ output
```

**Text output:**

```
Budget Report
=============
Texture2D (limit: 4.0 MB)
  /Game/Textures/T_HighRes_D    5.2 MB  [+1.2 MB]
  /Game/Textures/T_HighRes_N    4.8 MB  [+0.8 MB]

SoundWave (limit: 2.0 MB)
  /Game/Audio/S_Explosion       3.1 MB  [+1.1 MB]

Summary: 3 violations across 2 asset types.
```

**JSON output:**

```json
{
  "violations": [
    {"asset_path": "...", "asset_type": "Texture2D", "file_size": 5452595, "max_size": 4194304}
  ],
  "total": 3
}
```

**Exit codes:** violations found → 1; clean → 0; execution error → 2.

## Requirements

- [ ] Implement `budget` command handler
- [ ] Call `budget_tracker::check_budget()` with loaded config
- [ ] Implement text output grouped by asset type with excess MB shown
- [ ] Implement JSON output (violations array + total)
- [ ] Exit code 1 when violations found, 0 when clean

## Related

- Depends on: #12 (budget-tracker)
