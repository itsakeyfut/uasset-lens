# `baseline` Command — Internal Design

## Execution Flow: `baseline save`

```
1. AssetDb::open(db_path)                              [asset-db]
2. Check if <NAME>.json already exists in baselines dir
   └── If exists and not --yes: prompt "Overwrite? [y/N]"
   └── Decline → exit 0 without saving
3. Load all assets from DB
4. LintEngine::run_all(&assets, &config)               [lint-engine]
   └── collect Vec<Violation>
5. BudgetTracker::check_all(&assets, &config)          [budget-tracker]
   └── collect Vec<Violation>
6. Merge violations; compute summary (errors, warnings)
7. Detect git_commit via `git rev-parse --short HEAD`  [cli]
8. Serialize BaselineV1 to JSON
9. Write to .uasset-lens/baselines/<NAME>.json
10. Print confirmation line to stdout
11. Exit 0
```

## Execution Flow: `baseline diff`

```
1. AssetDb::open(db_path)                              [asset-db]
2. Read .uasset-lens/baselines/<NAME>.json
   └── File missing → exit 2 with error message
3. Load all assets from DB
4. Run lint + budget checks (same as save steps 4–6)
5. Compute regressions:
   baseline_set = HashSet<(rule, asset_path)> from baseline.violations where severity == "error"
   current_set  = HashSet<(rule, asset_path)> from current violations where severity == "error"
   regressions  = current_set − baseline_set
   resolved     = baseline_set − current_set
6. Render output to stdout
7. Exit 1 if regressions non-empty, else exit 0
```

## Execution Flow: `baseline list`

```
1. Resolve baselines_dir = <project_dir>/.uasset-lens/baselines/
2. If directory missing → print "No baselines saved." → exit 0
3. Read *.json files; deserialize each into BaselineV1
   └── Malformed files: log WARN, skip
4. Sort by saved_at descending
5. Render table; exit 0
```

## Execution Flow: `baseline delete`

```
1. Resolve path = .uasset-lens/baselines/<NAME>.json
2. If not exists → exit 2 with error message
3. If not --yes: prompt "Delete baseline '<NAME>'? [y/N]"
   └── Decline → exit 0
4. fs::remove_file(path)
5. Print confirmation; exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset loading | `uasset-lens-asset-db` |
| Lint violations | `uasset-lens-lint-engine` |
| Budget violations | `uasset-lens-budget-tracker` |
| Baseline file I/O, set-diff logic, rendering | `uasset-lens-cli` |

## Key Data Structures

```rust
struct BaselineV1 {
    version:    u32,
    name:       String,
    saved_at:   DateTime<Utc>,
    git_commit: Option<String>,
    summary:    ViolationSummary,
    violations: Vec<BaselineViolation>,
}

struct BaselineViolation {
    rule:       String,
    asset_path: String,
    severity:   Severity,  // "error" | "warning"
}

struct ViolationSummary {
    errors:   usize,
    warnings: usize,
}
```

## Regression Set Difference

Regression detection operates only on the `(rule, asset_path)` identity key. The
violation message is not part of the key — a violation with the same rule and asset
but a different message is considered the same violation (not a regression).

Warnings are included in the baseline file for informational display but are excluded
from the set-difference computation that determines exit code `1`.
