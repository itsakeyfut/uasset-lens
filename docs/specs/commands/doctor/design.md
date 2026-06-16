# `doctor` Command — Internal Design

## Execution Flow

```
1. Print "uasset-lens v{VERSION}" header to stdout

2. [DB check]
   └── db_path = project_dir / ".uasset-lens" / "uasset-lens.db"
       (or --db override)
   └── AssetDb::open(db_path)                   [asset-db]
       ├── Ok(db)  → [DB] ✓  record schema version + asset count
       └── Err(_)  → [DB] ✗  record error message; mark DB as missing

3. [Schema check]  (skipped if DB missing → mark as N/A)
   └── db.schema_version()                      [asset-db]
   └── compare to CURRENT_SCHEMA_VERSION constant
       ├── equal   → [Schema] ✓
       └── differs → [Schema] ✗  include migration hint

4. [Config check]
   └── config_path = project_dir / ".uasset-lens.toml"
       (or --config override)
   └── if not found:
       └── [Config] —  "not found (defaults will be used)"
   └── if found:
       └── Config::load(config_path)            [cli]
           ├── Ok(_)  → [Config] ✓
           └── Err(e) → [Config] ✗  include error location

5. [Scan check]  (skipped if DB missing → mark as N/A)
   └── db.last_scan_time()                      [asset-db]
       → SELECT MAX(scanned_at) FROM scan_history
   └── if None: [Scan] ✗  "No scan data available."
   └── if Some(t):
       └── days_since = (now_utc − t).num_days()
       └── [Scan] ✓  print timestamp + days_since

6. [Compat check]  (skipped if DB missing → mark as N/A)
   └── db.scanner_version()                     [asset-db]
       → SELECT scanner_version FROM db_meta
   └── compare to env!("CARGO_PKG_VERSION")
       ├── equal   → [Compat] ✓
       └── differs → [Compat] ✗  show DB version vs binary version

7. Count issues (checks with ✗ status)
8. Print summary line: "All checks passed." or "{N} issue(s) found."
9. exit 0 if issues == 0, else exit 1
```

---

## Crate Responsibilities

| Step | Crate |
|---|---|
| DB open + schema version query | `uasset-lens-asset-db` (`AssetDb::open`, `schema_version`) |
| Last scan time query | `uasset-lens-asset-db` (`last_scan_time`) |
| Scanner version query | `uasset-lens-asset-db` (`scanner_version`) |
| Config parse | `uasset-lens-cli` (`Config::load`) |
| Output formatting (text + JSON) | `uasset-lens-cli` |

---

## Check Result Model

```rust
enum CheckStatus {
    Pass(String),   // human-readable detail
    Fail(String),   // human-readable detail + optional hint
    Na(String),     // reason why check was skipped
}

struct DoctorReport {
    db:     CheckStatus,
    schema: CheckStatus,
    config: CheckStatus,
    scan:   CheckStatus,
    compat: CheckStatus,
}
```

The report is built sequentially. Each check receives the result of the previous check
so that dependent checks can be marked `Na` when a prerequisite failed.

---

## DB Meta Table

`scanner_version` and `schema_version` are read from a `db_meta` table that every
`AssetDb` maintains:

```
db_meta: (key TEXT PRIMARY KEY, value TEXT)
  key = "schema_version"  → e.g. "4"
  key = "scanner_version" → e.g. "0.2.0"
```

`scanner_version` is written during every `scan` run and reflects the binary version
that last wrote to the DB.

---

## CURRENT_SCHEMA_VERSION

The constant is defined in `uasset-lens-asset-db` and must be incremented whenever a
migration changes the DB schema. `doctor` compares the stored version against this
constant to detect both upgrades (stored < current) and downgrades (stored > current).
Both cases produce a `[Schema] ✗` result.
