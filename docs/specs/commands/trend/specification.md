# `trend` Command — Specification

## Purpose

Show how key asset metrics have changed over time by reading stored scan history and
saved baselines. Useful for spotting regressions or improvements across multiple
development sessions.

```bash
uasset-lens trend ./Project
uasset-lens trend ./Project --metric violations,dead-assets
```

Requires at least 2 history entries to display a trend. Reads
`.uasset-lens/history/*.json` and `.uasset-lens/baselines/*.json`.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Trend displayed successfully (including insufficient data case) |
| `2` | Execution error (I/O failure, DB error) |

---

## Available Metrics

| Metric key | Source | Description |
|---|---|---|
| `assets` | history snapshot | Total asset count after scan |
| `violations` | saved baselines | Error-level violation count (requires `baseline save`) |
| `dead-assets` | DB query per history entry | Count of unreferenced assets |
| `cycles` | DB query per history entry | Count of dependency cycles |
| `file-size-total` | history snapshot | Sum of all asset file sizes in bytes |

When `--metric` is omitted, all five columns are shown. Metrics with no data available
show `—` in the column.

---

## Text Output

```
Asset Trends (./Project, last 7 scans)

Date              Assets  Violations  Dead  Cycles  Total Size
2026-06-16 14:23  1,024   5           47    2       4.2 GB
2026-06-15 09:11  1,021   3           48    2       4.1 GB
2026-06-14 16:45  1,022   3           49    3       4.1 GB
2026-06-13 11:30  1,014   3           50    3       4.0 GB
2026-06-12 09:00  1,010   2           51    4       3.9 GB
2026-06-11 15:20  1,008   2           52    4       3.9 GB
2026-06-10 10:45  1,005   —           53    5       3.8 GB

Trend: assets ↑ +19, violations ↑ +3, dead ↓ -6, cycles ↓ -3, size ↑ +0.4 GB
```

The `Trend` summary line compares the most recent entry against the oldest entry in
the displayed window. Direction arrows: `↑` = increased, `↓` = decreased, `→` = unchanged.

`violations` column shows `—` for rows where no saved baseline exists for that date.

When fewer than 2 history entries exist:

```
Insufficient history to display trends (need at least 2 scans).
Run 'uasset-lens scan' to record more history.
```

---

## JSON Output (`--format json`)

```json
{
  "project": "./Project",
  "entries": [
    {
      "date": "2026-06-16T14:23:01Z",
      "metrics": {
        "assets": 1024,
        "violations": 5,
        "dead_assets": 47,
        "cycles": 2,
        "file_size_total": 4509715456
      }
    }
  ],
  "trend": {
    "assets":          { "delta": 19,        "direction": "up" },
    "violations":      { "delta": 3,         "direction": "up" },
    "dead_assets":     { "delta": -6,        "direction": "down" },
    "cycles":          { "delta": -3,        "direction": "down" },
    "file_size_total": { "delta": 429496729, "direction": "up" }
  }
}
```

Null values in `metrics` (when data is unavailable) are represented as `null`.

---

## Error Cases

| Condition | Behaviour |
|---|---|
| History directory missing | Print "Insufficient history..." message; exit 0 |
| Fewer than 2 valid history entries | Print "Insufficient history..." message; exit 0 |
| DB not found | Exit `2` with error (DB needed for `dead-assets` and `cycles` columns) |
