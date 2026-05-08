# `crates/lint-engine` — Blueprint complexity lint rules

## Summary

Implement the Blueprint complexity lint rules that check `BlueprintMetrics` against
configurable thresholds.
Complete when a Blueprint exceeding node count or EventTick limits produces violations.

## Design Notes

**Rule:**

```rust
pub struct BlueprintComplexityRule {
    pub thresholds: ComplexityThresholds,   // from bp-analyzer
}
```

`check()`: call `bp_analyzer::is_complex(metrics, &self.thresholds)` if `metrics` is `Some`.
Convert each returned `Warning` into a `LintViolation` with `Error` severity.

Return empty Vec if `metrics` is `None` (non-Blueprint asset).

**Violation details:**

```
rule_id: "blueprint/node-count"
message: "BP_Player has 312 nodes (limit: 200)"

rule_id: "blueprint/event-tick"
message: "BP_Enemy uses EventTick (limit: 1)"
```

## Requirements

- [ ] Implement `BlueprintComplexityRule` struct implementing `LintRule`
- [ ] `check()` returns empty Vec when `metrics` is `None`
- [ ] Delegate threshold checking to `bp_analyzer::is_complex()`
- [ ] Map each `Warning` to a `LintViolation` with `Error` severity
- [ ] Unit test: Blueprint metrics below all thresholds → no violations
- [ ] Unit test: node count above threshold → violation with rule_id `"blueprint/node-count"`
- [ ] Unit test: EventTick count above threshold → violation with rule_id `"blueprint/event-tick"`
- [ ] Unit test: non-Blueprint asset (metrics = None) → no violations

## Related

- Depends on: #6 (LintRule trait), #3 (bp-analyzer ComplexityThresholds)
- Next: #10 — [lint] config section
