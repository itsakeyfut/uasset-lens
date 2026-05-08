# `crates/impact-analyzer` — Phase 2 stub

## Summary

Create the `impact-analyzer` crate as a thin re-export layer over `dependency-graph`.
Complete when the crate compiles and `ImpactResult` is accessible via `impact_analyzer::ImpactResult`.

## Design Notes

In Phase 2, the CLI calls `dependency_graph::DependencyGraph::find_impact()` directly.
This crate exists as an extension point for Phase 3+ enhancements (e.g., Soft Reference
analysis, rename safety checks) without requiring CLI changes.

**`src/lib.rs`:**

```rust
pub use dependency_graph::ImpactResult;
```

Nothing else for now.

## Requirements

- [ ] Create `crates/impact-analyzer` crate with `dependency-graph` as a dependency
- [ ] Re-export `ImpactResult` from `dependency_graph` in `lib.rs`
- [ ] Crate compiles with no warnings (`cargo build -p impact-analyzer`)

## Related

- Depends on: Phase 2 Issue #3 (ImpactResult defined in dependency-graph)
- Future: Phase 3+ will add Soft Reference analysis here
