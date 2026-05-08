# `crates/lint-engine` — LintRule trait and LintViolation struct

## Summary

Create the `lint-engine` crate with the `LintRule` trait and `LintViolation` struct
that all lint rules implement.
Complete when the crate compiles and a minimal no-op rule can be wired up.

## Design Notes

**Core types:**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

pub struct LintViolation {
    pub severity:   Severity,
    pub rule_id:    &'static str,
    pub message:    String,
    pub asset_path: AssetPath,
}

pub trait LintRule: Send + Sync {
    fn rule_id(&self) -> &'static str;
    fn check(
        &self,
        asset: &AssetRecord,
        metrics: Option<&BlueprintMetrics>,
    ) -> Vec<LintViolation>;
}
```

The `check()` method receives both the DB record (for type, size, path) and optional
Blueprint metrics (populated only for Blueprint-type assets).

**`LintEngine` runner:**

```rust
pub struct LintEngine {
    rules: Vec<Box<dyn LintRule>>,
}

impl LintEngine {
    pub fn new(rules: Vec<Box<dyn LintRule>>) -> Self
    pub fn run(&self, assets: &[AssetRecord], metrics_map: &HashMap<AssetPath, BlueprintMetrics>) -> Vec<LintViolation>
}
```

`run()` iterates all assets × all rules and collects violations.

## Requirements

- [ ] Create `crates/lint-engine` crate
- [ ] Define `Severity` enum
- [ ] Define `LintViolation` struct
- [ ] Define `LintRule` trait
- [ ] Define `LintEngine` struct with `new()` and `run()`
- [ ] Unit test: `LintEngine::new(vec![])` with no rules produces empty violations for any input
- [ ] Crate compiles with no warnings

## Related

- Next: #7 — naming prefix rules (first concrete rule implementation)
- Used by: #14 (lint command)
