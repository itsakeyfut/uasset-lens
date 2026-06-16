# Soft Reference Cycle Detection — Specification

## Background

UE5 has two categories of asset reference:

| Type | How Stored | Currently Tracked |
|---|---|---|
| Hard reference | Import table (`FObjectImport`) | Yes — existing dependency extraction |
| Soft reference | `FSoftObjectPath` / `TSoftObjectPtr` string fields in export data | No — this spec |

Soft references are not captured by the standard import table extraction because they are
stored as serialized `FString` values inside export properties, not as import table entries.
A soft reference cycle is therefore invisible to the existing `graph --cycles-only` command.

---

## Soft Reference Extraction

### Parser Extension (`uasset-lens-scanner`)

After completing the import table extraction pass, scan all `FString` export property values
for patterns that match asset paths:

```
/Game/[A-Za-z0-9_./-]+
```

Path normalization:
- Strip a trailing `.uasset` or `.umap` extension if present.
- Normalize path separators to `/`.
- Discard matches shorter than `/Game/A` (minimum meaningful path).

Only scan `StrProperty` and `SoftObjectProperty` tagged values; skip bulk data blocks to
avoid false positives from serialized binary content.

### Storage

Soft references are stored in a separate table to distinguish them from hard references.

```sql
CREATE TABLE soft_dependencies (
    from_path TEXT NOT NULL,
    to_path   TEXT NOT NULL,
    PRIMARY KEY (from_path, to_path),
    FOREIGN KEY (from_path) REFERENCES assets(path) ON DELETE CASCADE
);
```

The `to_path` may reference an asset that is not in the `assets` table (stale or
external reference). This is not an error; the row is retained.

---

## Soft Reference Cycle Detection

### Algorithm

Cycle detection operates on the combined graph: hard edges (from `dependencies` table) plus
soft edges (from `soft_dependencies` table). A cycle that exists only via soft edges is
still a cycle.

Use the existing strongly-connected component (SCC) algorithm in `uasset-lens-dependency-graph`.
A new graph builder variant accepts an `include_soft: bool` parameter to optionally merge
both edge sets before running SCC.

### Command

```bash
uasset-lens graph ./Project --soft-refs --cycles-only
```

`--soft-refs` adds soft dependency edges to the graph before analysis.
`--cycles-only` filters output to SCCs of size ≥ 2.

Without `--soft-refs`, the command behaves as today (hard references only).

### `check` Integration

When `soft-ref-cycles` is listed in the enabled checks:

```toml
[check]
enabled = ["soft-ref-cycles"]
```

The `check` command runs cycle detection on the combined graph and reports any cycles that
include at least one soft edge. Cycles composed entirely of hard edges are reported by the
existing `cycles` check and are not duplicated here.

---

## Dead Asset Interaction

By default, `dead-assets` considers only hard references when determining reachability.
An asset reachable only via a soft reference is still classified as dead, because soft
references may be stale string literals that no longer resolve.

```bash
uasset-lens dead-assets ./Project --include-soft-refs
```

With `--include-soft-refs`, assets reachable through soft references are excluded from the
dead asset result set. This flag is opt-in and off by default.

---

## Performance Considerations

String scanning of all export property data adds overhead to the scan pass. Benchmarks
should verify that the 1,000-asset-in-5-second target is not violated. If scanning is too
slow, soft reference extraction can be gated behind a `--extract-soft-refs` flag on the
`scan` command rather than running by default.

The `soft_dependencies` table is indexed on `from_path` for fast lookup during graph
construction.

```sql
CREATE INDEX idx_soft_deps_from ON soft_dependencies(from_path);
CREATE INDEX idx_soft_deps_to   ON soft_dependencies(to_path);
```

---

## Limitations

- Soft references computed at runtime (e.g., string concatenation in Blueprint) cannot be
  detected by static scanning.
- `TSoftClassPtr` (class references, not asset references) may produce false positives if
  the class path matches the `/Game/` prefix pattern; these are filtered by checking whether
  the resolved path exists in the `assets` table.
- Soft references inside `DataTable` row struct fields are not scanned in MVP. The row data
  blob is treated as opaque bulk data.
