# `crates/redirector-analyzer` — detect ObjectRedirector assets

## Summary

Create the `redirector-analyzer` crate and implement `detect()`, which collects all
`ObjectRedirector` assets from the dependency graph.
Complete when `detect()` returns the expected redirector paths from a test graph.

## Design Notes

`ObjectRedirector` is UE's mechanism for keeping old asset references valid after a rename.
Detecting them is useful for identifying cleanup opportunities in the project.

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>
```

Iterate `graph.nodes()`, collect paths where `node.asset_type == AssetType::ObjectRedirector`.

This is a pure function — no DB, no IO. Depends only on `dependency-graph`.

> **Note**: Phase 1 only detects the existence of redirectors. Resolving *where* they redirect
> to (the redirect target path) requires Export property parsing from Phase 4.
> The CLI output should include a note: `"Note: redirect target resolution is available in Phase 4 analysis."`

## Requirements

- [ ] Create `crates/redirector-analyzer` crate depending on `dependency-graph`
- [ ] Implement `detect(graph: &DependencyGraph) -> Vec<AssetPath>`
- [ ] Collect nodes where `asset_type == AssetType::ObjectRedirector`
- [ ] Unit test: graph with no redirectors → empty result
- [ ] Unit test: graph with only redirectors → all returned
- [ ] Unit test: mixed graph → only redirector nodes returned

## Related

- Used by: Issue #3 (redirectors command)
- Docs: `docs/roadmap/phase3/ROADMAP.md` — Task 1
