# `audit` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                              [asset-db]
2. Resolve asset_path to canonical game path
3. db.get_asset(asset_path)                            [asset-db]
   └── AssetNotFound → exit 2 with hint to run scan
4. Build dependency list:
   └── DependencyGraph::from_db(&db)                  [dependency-graph]
   └── graph.direct_deps(asset_path)
5. Build impact list:
   └── graph.direct_referrers(asset_path)
6. Run lint checks:
   └── LintEngine::run_single(asset, &config)         [lint-engine]
   └── collect Violation { rule, message, severity }
7. Run budget checks:
   └── BudgetTracker::check_single(asset, &config)    [budget-tracker]
   └── collect Violation { rule, message, severity }
8. Resolve flags:
   └── referenced: in_degree > 0
   └── redirector: asset.asset_type == ObjectRedirector
   └── in_ignore_list: config.ignore_paths contains asset_path
9. Render output (text or JSON) to stdout
10. Exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset metadata lookup | `uasset-lens-asset-db` |
| Dependency and impact lists | `uasset-lens-dependency-graph` |
| Lint violations | `uasset-lens-lint-engine` |
| Budget violations | `uasset-lens-budget-tracker` |
| Flag resolution and rendering | `uasset-lens-cli` |

## Key Data Structures

```rust
// Assembled in the CLI layer before rendering
struct AuditReport {
    asset:       AssetMetadata,
    deps:        Vec<AssetRef>,         // (path, type)
    referrers:   Vec<AssetRef>,
    violations:  Vec<Violation>,
    flags:       AuditFlags,
}

struct AuditFlags {
    in_degree:      usize,
    redirector:     bool,
    in_ignore_list: bool,
}
```

## List Truncation

Text output truncates both `deps` and `referrers` to the first 10 entries and appends
`... (N more)` when the total exceeds 10. JSON output never truncates; it emits all
entries in `items` regardless of count.

## Violation Ordering

Violations are sorted: errors first, then warnings, each group sorted by rule name
lexicographically.

## Dependency Graph Scope

`audit` loads the full dependency graph from the DB to resolve direct deps and direct
referrers. This is the same graph used by `deps` and `impact`. Building the full graph
is acceptable here because `audit` is a per-asset interactive command, not a batch
operation.
