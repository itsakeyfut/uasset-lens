# DataTable and CurveTable Analysis — Specification

## Purpose

Analyze `DataTable` and `CurveTable` assets to extract row/curve counts and the row struct
type. Enables lint rules that flag excessively large tables and missing row struct
resolution, and enables impact analysis for struct-to-table relationships.

---

## Scope

| Asset Class | Covered |
|---|---|
| `DataTable` | Yes — row count and row struct |
| `CurveTable` | Yes — curve count |

---

## Metadata Extracted (Scanner Additions)

### DataTable

Fields are read from the `DataTable` export property stream.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `row_count` | `int32` | `RowMap` map property entry count | Number of data rows in the table |
| `row_struct` | `FString` | Import table entry for the `RowStruct` object reference | Fully qualified class path of the `UScriptStruct` used as the row type |

### CurveTable

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `curve_count` | `int32` | `CurveTableMode`-dependent row map entry count | Number of named curves in the table |

---

## Database Schema

```sql
CREATE TABLE data_table_metadata (
    asset_path TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    asset_type TEXT     NOT NULL,  -- 'DataTable' or 'CurveTable'
    row_count  INTEGER,            -- row count for DataTable; curve count for CurveTable
    row_struct TEXT                -- NULL for CurveTable; import path for DataTable
);
```

`row_count` stores the row count for `DataTable` and the curve count for `CurveTable`
to keep the schema unified. Queries that need to distinguish them use `asset_type`.

---

## Dependency Extraction

`DataTable` references its `RowStruct` via a `StructProperty` import, which is captured by
the existing import table extraction. This means:

- The row struct type (`/Script/MyGame.FMyItemData`) appears as a dependency of the
  `DataTable` asset.
- Impact analysis can answer: "which data tables use struct `FMyItemData`?" by querying
  reverse dependencies from the struct import path.

No special parser extension is required for this relationship — it falls out of the standard
dependency extraction pass.

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/data-table/large-table` | Warning | `row_count > 10000` | Very large data tables cause slow editor load times and should be split by domain or migrated to a runtime database |
| `lint/data-table/missing-struct` | Error | `asset_type = 'DataTable'` and `row_struct IS NULL` | A data table with an unresolvable struct type cannot be safely referenced at runtime |

---

## Budget Rules

```toml
[budget]
DataTable  = "1MB"   # per-file default; configurable in .uasset-lens.toml
CurveTable = "512KB"
```

---

## UE5 Binary Format Notes

`DataTable` stores rows in a `MapProperty` (`RowMap`) keyed by `FName`. The value type
is determined by the `RowStruct` import reference, serialized as a
`StructProperty` header before the map entries.

Row count is the number of entries in `RowMap`. This is a tagged `MapProperty` and its
count is encoded as a `uint32` immediately before the map element data.

`CurveTable` stores curves as a `MapProperty` (`CurveTableMode`-dependent) keyed by
`FName`. The curve count is the entry count of this map.

Both types use the same outer tagged property stream format. The class name in the export
table (`DataTable` vs. `CurveTable`) determines which extraction path to use.
