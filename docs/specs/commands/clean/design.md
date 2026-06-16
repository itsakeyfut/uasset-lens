# `clean` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                            [asset-db]
2. load_graph(&db, external_roots)                   [dependency-graph]
3. dead_asset_detector::detect(&graph, DEFAULT_EXCLUDED_TYPES)  [dead-asset-detector]
   └── same algorithm as `dead-assets` command
4. Build CleanEntry list (join with DB for file_path and last_modified_db)
5. Apply filter pipeline:
   a. --min-size <N>: retain entries where file_size >= N
   b. --exclude <PATTERN,...>: retain entries where game path does NOT contain pattern
   c. --path <GLOB>: retain entries matching globset pattern on game path
6. Sort by file_size DESC
7. If --dry-run: print list + total size summary, return 0 immediately
8. Print discovery summary to stderr
9. Interactive deletion loop (or --yes bypass):
   └── for each entry:
       a. if skip_types.contains(asset_type): skip
       b. Print entry header to stderr
       c. If !delete_all: check mtime vs last_modified_db (warn if changed)
       d. Prompt: [y / N / a(ll) / s(kip type) / q(uit)]
          └── 'a': delete_all = true (no more prompts)
          └── 's': skip_types.insert(asset_type)
          └── 'q': break loop
          └── 'y': proceed to delete
          └── default (N, empty): skip entry
       e. std::fs::remove_file(file_path)
       f. delete_sidecars(file_path)   → try .uexp, .ubulk, .uptnl
       g. db.delete_asset(asset_path)
10. Print summary: deleted / skipped / errors
11. Return 0 always (non-gate command)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Dead asset detection | `uasset-lens-dead-asset-detector` |
| DB record deletion | `uasset-lens-asset-db` |
| File deletion + sidecar cleanup | `uasset-lens-cli` |

## Sidecar Files

UE5 assets often have companion files with the same base name but different extensions.
These are deleted automatically after the primary `.uasset` is deleted:

| Extension | Contents |
|---|---|
| `.uexp` | Export payload (bulk serialized object data) |
| `.ubulk` | Optional bulk data overflow |
| `.uptnl` | Optional patching metadata |

Sidecar deletion failures emit warnings (`errors += 1`) but do not abort the loop.

## Safety Mechanisms

**Mtime validation**: Before each interactive prompt, the asset's current mtime is
compared against the value stored in the DB. A mismatch means the file was modified
since the last scan — the user is warned before being asked to confirm deletion.

**Type skipping**: Selecting `s` adds the current asset's type to `skip_types`. All
subsequent assets of that type are automatically skipped without prompting.

**Scope guarantee**: Only assets that `dead_asset_detector::detect()` would report
are eligible for deletion. Clean never deletes referenced assets.

## Interactive Prompt Design

```
  [  3/10] /Game/Characters/Old_BP_Enemy  (Blueprint, 24.0 KB)
  Delete? [y / N / a(ll) / s(kip type) / q(uit)] > _
```

The `[N/total]` counter uses zero-padded width (`ilog10(total) + 1`) for alignment.

## --dry-run

Returns 0 and prints the target list without touching any files or the DB. Useful
for reviewing what `clean` would do before committing to deletion.
