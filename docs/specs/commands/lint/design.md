# `lint` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                            [asset-db]
2. Build LintEngine:
   └── lint_builder::build_lint_rules(&cfg.lint)     [cli/lint_builder.rs]
   └── returns Vec<Box<dyn LintRule>>
   └── LintEngine::new(rules)                        [lint-engine]
3. db.all_assets() → Vec<AssetRecord>                [asset-db]
4. db.all_blueprint_metrics() → Vec<BlueprintMetricsRow>  [asset-db]
   └── build metrics_map: HashMap<AssetPath, BlueprintMetrics>
5. engine.run(&assets, &metrics_map)                 [lint-engine]
   └── for each rule, for each asset: rule.check(asset, metrics) → Option<LintViolation>
   └── returns Vec<LintViolation>
6. Budget check (always runs, in addition to lint rules):
   └── BudgetConfig::effective(&cfg.budget)          [budget-tracker]
   └── check_budget(&assets, &config)                [budget-tracker]
   └── convert BudgetViolation → LintViolation (Severity::Warning)
7. Merge budget violations into lint violations list
8. Format and output:
   └── text:           tabular (severity, rule_id, asset_path, message)
   └── json:           array of LintEntry objects
   └── github-actions: ::error or ::warning per violation; exit 1 only on Error severity
9. Return: 1 if any violations, 0 if clean
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Rule construction | `uasset-lens-cli` (lint_builder.rs) |
| Rule execution | `uasset-lens-lint-engine` |
| Blueprint metrics lookup | `uasset-lens-asset-db` |
| Budget check | `uasset-lens-budget-tracker` |
| Annotation output | `uasset-lens-cli` |

## LintEngine Architecture

```
LintEngine
├─ NamingPrefixRule   (Blueprint → BP_, Texture2D → T_, etc.)
├─ BlueprintComplexityRule  (node_count > threshold → error)
└─ [future rules via Box<dyn LintRule>]
```

`engine.run()` calls each rule against each asset in a nested loop.
Rules return `None` if the asset passes, or `Some(LintViolation)` if it fails.

## Budget Violations as Lint Items

Budget violations are deliberately merged into the lint violation list rather than
handled separately. This gives a single unified output for CI consumption.

Budget violations are always `Severity::Warning` (not `Error`). They are identified
by `rule_id` of the form `budget/<type_lowercase>` (e.g. `budget/texture2d`).

## github-actions Exit Code Distinction

In `--format github-actions` mode, the exit code depends only on `Severity::Error`
violations:

- Warnings only → `::warning` annotations, exit 0
- Any error → `::error` annotations, exit 1

This allows CI pipelines to gate on naming errors while only surfacing budget
warnings without failing the build.

## `maybe_hint_github_actions`

A helper function printed to stderr when running in `Text` format suggests
`--format github-actions` to the user. This hint is suppressed in JSON mode.
