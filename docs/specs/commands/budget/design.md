# `budget` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                           [asset-db]
2. db.all_assets() → Vec<AssetRecord>               [asset-db]
3. BudgetConfig::effective(&cfg.budget)             [budget-tracker]
   └── merges user config with built-in defaults
   └── built-in default: Texture2D = 4 MB
4. budget_tracker::check_budget(&assets, &config)   [budget-tracker]
   └── for each asset: compare file_size vs config[asset_type].max_size
   └── returns BudgetReport { violations: Vec<BudgetViolation> }
5. Map violations to Vec<BudgetViolationEntry>
6. Format and output:
   └── text: group by asset type, show excess per asset
   └── json: { violations: [...], total: N }
   └── github-actions: one ::error annotation per violation
7. Return: 1 if violations exist, 0 if all within budget
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Budget config resolution | `uasset-lens-budget-tracker` |
| Per-asset size check | `uasset-lens-budget-tracker` |
| GitHub Actions annotation | `uasset-lens-cli` |

## Config Resolution (`BudgetConfig::effective`)

The effective budget merges built-in defaults with user overrides:

```toml
# Built-in defaults (always active)
[budget]
Texture2D.max_size = 4194304     # 4 MB

# User config overrides
[budget]
Texture2D.max_size = 8388608     # override to 8 MB
StaticMesh.max_size = 10485760   # add limit for StaticMesh
```

Types not in the config have no budget constraint and are not checked.

## Text Output Grouping

In text mode, violations are grouped by asset type using a `BTreeMap` (alphabetical
order). Each group shows the type name with its limit, then all violating assets with
their actual size and excess:

```
Texture2D (limit: 4.0 MB)
  /Game/Environments/T_LargeTerrain   5.2 MB  [+1.2 MB]
  /Game/Characters/T_HighResArmor     6.8 MB  [+2.8 MB]
```

## github-actions Annotation Severity

Budget violations are emitted as `::error` annotations (the highest severity level),
causing the GitHub Actions step to be marked as failed in the PR check view. This
distinguishes budget from blueprint complexity (which uses `::notice`).
