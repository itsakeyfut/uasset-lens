# `check` Command — Internal Design

## Execution Flow

```
uasset-lens check <project_dir>
         │
         ▼
1. Resolve project_dir and content_root
         │
         ▼
2. Load .uasset-lens.toml (or defaults)
         │
         ▼
3. [unless --skip-scan] mtime delta scan → update DB
         │
         ▼
4. Run enabled checks (DB read-only after this point)
   ├─ [rules] dead-assets       → uasset-lens-dead-asset-detector
   ├─ [rules] circular-deps     → uasset-lens-dependency-graph
   ├─ [rules] duplicate-assets  → uasset-lens-duplicate-detector
   ├─ [rules] redirectors       → uasset-lens-redirector-analyzer
   ├─ [lint]  naming.*          → uasset-lens-lint-engine
   ├─ [lint]  blueprint.*       → uasset-lens-bp-analyzer + uasset-lens-lint-engine
   └─ [budget] <Type>.*         → uasset-lens-budget-tracker
         │
         ▼
5. [if --diff-from] load baseline JSON, compute regression set
         │
         ▼
6. Format and write output (text / json / github-actions)
         │
         ▼
7. [if --save-baseline] serialize violations to baseline JSON
         │
         ▼
8. exit 0 / 1 / 2
```

---

## Crate Responsibilities

| Check | Crate |
|---|---|
| `dead-assets` | `uasset-lens-dead-asset-detector` |
| `circular-deps` | `uasset-lens-dependency-graph` (Tarjan SCC) |
| `duplicate-assets` | `uasset-lens-duplicate-detector` |
| `redirectors` | `uasset-lens-redirector-analyzer` |
| `lint.naming.*` | `uasset-lens-lint-engine` |
| `lint.blueprint.*` | `uasset-lens-bp-analyzer` + `uasset-lens-lint-engine` |
| `budget.*` | `uasset-lens-budget-tracker` |

All checks read from `uasset-lens-asset-db` (SQLite). None modify it.

---

## Violation Data Model

```rust
pub struct Violation {
    pub severity: Severity,   // Error | Warn
    pub rule: String,         // e.g. "blueprint.event_tick_limit"
    pub asset_path: String,   // e.g. "/Game/Characters/BP_Player"
    pub file: String,         // e.g. "Content/Characters/BP_Player.uasset"
    pub message: String,      // e.g. "EventTick node count (8) exceeds limit (5)"
}

pub enum Severity { Error, Warn }
```

---

## Baseline JSON Schema

```json
{
  "version": 1,
  "git_commit": "abc1234",
  "summary": {
    "errors": 3,
    "warnings": 12
  },
  "violations": [
    {
      "severity": "error",
      "rule": "blueprint.event_tick_limit",
      "asset_path": "/Game/Characters/BP_Player",
      "message": "EventTick node count (8) exceeds limit (5)"
    }
  ]
}
```

- `version`: schema version, currently `1`. Breaking changes increment this.
- `git_commit`: output of `git rev-parse HEAD`, empty string if git is unavailable.
- `file` is omitted from the baseline to keep comparisons path-stable across machines.
- Matching uses `rule` + `asset_path` only — `message` changes do not affect matching.

---

## Regression Detection Algorithm

```
baseline_set = { (rule, asset_path) for v in baseline.violations }
current_set  = { (rule, asset_path) for v in current.violations if severity == Error }

regressions = current_set - baseline_set   → new violations (fail)
resolved    = baseline_set - current_set   → fixed violations (no fail)
unchanged   = current_set ∩ baseline_set   → pre-existing (no fail)
```

Exit `1` if `regressions` is non-empty.

`"warn"` violations are never included in regression detection — only `"error"`.

---

## Config → Check Mapping

The `check` command reads `.uasset-lens.toml` and builds a `CheckConfig`:

```rust
pub struct CheckConfig {
    pub dead_assets: Severity,      // off → skip
    pub circular_deps: Severity,
    pub duplicate_assets: Severity,
    pub redirectors: Severity,
    pub lint: LintConfig,
    pub budget: BudgetConfig,
}
```

A rule with `"off"` severity is skipped entirely — its crate is never called.

---

## `--format github-actions` Implementation

Each violation is rendered as a workflow command:

```
::{level} file={file},title={rule}::{message}
```

Where:
- `{level}`: `error` for `Severity::Error`, `warning` for `Severity::Warn`
- `{file}`: `violation.file` (relative to repository root)
- `{rule}`: `violation.rule`
- `{message}`: `violation.message`

All annotations are written to stdout. Progress messages (scanning, running checks) are
suppressed when `--format github-actions` is active to avoid polluting the annotation stream.
