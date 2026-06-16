# Asset Health Score — Specification

## Purpose

Provide a single 0–100 metric per asset summarizing its quality relative to configured
budgets and lint rules. The score allows fast triage (`--sort health`) and project-wide
quality tracking (`stats` command).

---

## Score Formula

```
health = clamp(100 - error_penalty - warning_penalty - budget_penalty, 0, 100)
```

### Error Penalty

```
error_penalty = min(error_count * 20, 60)
```

Each lint rule violation at severity `Error` subtracts 20 points. Capped at −60 total
(three or more errors all score the same floor from errors alone).

### Warning Penalty

```
warning_penalty = min(warning_count * 5, 20)
```

Each lint rule violation at severity `Warning` subtracts 5 points. Capped at −20 total.

### Budget Penalty

Applied when the asset's file size exceeds the configured budget for its type.

| Condition | Additional Penalty |
|---|---|
| File size > 2× budget | −10 |
| File size > 5× budget | −20 (replaces the 2× penalty, not additive) |

### Info Violations

Lint violations at severity `Info` do not affect the health score.

---

## Score Ranges

| Score | Label | Description |
|---|---|---|
| 90–100 | Excellent | No or very minor issues |
| 70–89 | Good | Some warnings; no errors |
| 50–69 | Fair | Errors present; needs attention |
| 0–49 | Poor | Severe violations or significant budget overrun |

---

## Computation

The health score is computed at query time from violation and budget data stored in the
database. It is **not stored** — recomputing it is cheap and keeps the score consistent
with the currently configured thresholds without requiring a rescan.

The computation is performed in the CLI layer (`uasset-lens-cli`) and is not part of any
analyzer crate.

---

## CLI Integration

### `audit <ASSET>`

Displays the computed health score alongside violation details:

```
$ uasset-lens audit /Game/Characters/SK_Hero

Health: 65 / 100  [Fair]
  2 errors, 1 warning, 0 budget violations

  ERROR   lint/skeletal-mesh/no-lod         (lod_count=1, triangles=18000)
  ERROR   lint/skeletal-mesh/high-poly-morph (triangles=18000, has_morph=true)
  WARNING lint/skeletal-mesh/excess-bones    (bone_count=312)
```

### `stats`

Displays project-wide average health:

```
$ uasset-lens stats ./Project

Assets: 1,022   Average health: 81.4 / 100
  Excellent (90–100): 712 assets
  Good      (70–89):  198 assets
  Fair      (50–69):   87 assets
  Poor      ( 0–49):   25 assets
```

### `find --sort health`

Returns assets sorted by health score ascending (worst first):

```
$ uasset-lens find --type Texture2D --sort health

  45  /Game/Textures/T_LegacyUncompressed4K  [Poor]
  60  /Game/Characters/T_HeroAlpha           [Fair]
  72  /Game/UI/T_ButtonNormal                [Good]
  ...
```

---

## JSON Output

When `--format json` is active, health score is included as a top-level field in
per-asset result objects:

```json
{
  "path": "/Game/Characters/SK_Hero",
  "type": "SkeletalMesh",
  "health": 65,
  "health_label": "Fair"
}
```
