# `crates/cli` — `load_graph()` helper and `graph` command

## Summary

Implement the `load_graph()` shared helper that builds a `DependencyGraph` from the DB,
and the `graph` command that displays dependency graph statistics and cycles.
Complete when `uasset-lens graph ./Project` outputs cycle information and exits with
code 1 when cycles are present.

## Design Notes

**`load_graph()` helper** (defined in `cli` crate, reused by all Phase 2 commands):

```rust
fn load_graph(db: &AssetDb) -> Result<DependencyGraph> {
    let records = db.all_assets()?;
    let nodes: Vec<AssetNode> = records.iter()
        .map(|r| AssetNode { path: r.asset_path.clone(), asset_type: r.asset_type.clone() })
        .collect();
    let edges = db.all_edges()?;
    Ok(DependencyGraph::build(nodes, edges))
}
```

If the DB file does not exist: print `"Run 'uasset-lens scan <project>' first."` to stderr and exit 2.

**`graph` command text output (from `docs/specs/cli-design.md`):**

```
Dependency Graph
================
Total assets : 523
Total edges  : 1 204
Cycles       : 2

Cycle 1:
  /Game/Characters/BP_Player
  /Game/Characters/BP_Enemy
  /Game/Characters/BP_Player   ← (back to start)

Cycle 2:
  ...
```

**`--cycles-only` flag:** skip the summary header, print only the cycle list.

**JSON output:**

```json
{"total_assets": 523, "total_edges": 1204, "cycles": [["/Game/A", "/Game/B"]]}
```

**Exit codes:** `--cycles-only` with cycles found → 1; no cycles → 0; execution error → 2.
Without `--cycles-only`: always 0 unless execution error.

## Requirements

- [ ] Implement `load_graph(db: &AssetDb) -> Result<DependencyGraph>` helper
- [ ] Handle missing DB file: print error to stderr, exit 2
- [ ] Implement `graph` command handler calling `load_graph()` + `find_cycles()`
- [ ] Implement text output (summary + cycle list) matching the spec
- [ ] Implement `--cycles-only` flag (omit summary, only print cycles)
- [ ] Implement JSON output for `--format json`
- [ ] Exit code 1 when `--cycles-only` and cycles found; 0 otherwise

## Related

- Depends on: Phase 2 Issues #1–#3 (DependencyGraph), Phase 1 Issue #11 (all_assets, all_edges)
- Next: #7 — dead-assets command
- Docs: `docs/specs/cli-design.md` (graph output spec)
