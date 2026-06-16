# `audit` Command — Specification

## Purpose

Display a full single-asset analysis report. Assembles everything known about one
asset: basic metadata, direct dependencies, reverse dependencies (impact), all active
lint and budget violations, and status flags.

```bash
uasset-lens audit ./Project /Game/Characters/BP_Player
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Report displayed successfully (even if violations exist) |
| `2` | Execution error (asset not found in DB, I/O failure, DB error) |

---

## Text Output

```
Asset: /Game/Characters/BP_Player
Type:  Blueprint (1.4 MB)
Path:  Content/Characters/BP_Player.uasset

Dependencies (3):
  /Game/Characters/SK_Player          (SkeletalMesh)
  /Game/Characters/T_Player_D        (Texture2D)
  /Game/GameModes/BP_GameMode        (Blueprint)

Referenced by (12):
  /Game/Levels/L_Main                (World)
  /Game/UI/WBP_HUD                   (WidgetBlueprint)
  ... (10 more)

Violations (2):
  [lint/blueprint/event-tick-count] EventTick nodes: 8 (limit: 5)  [ERROR]
  [lint/naming/blueprint-prefix]    Missing prefix 'BP_'            [ERROR]

Flags:
  Referenced: yes (12 assets)
  Redirector: no
  In ignore list: no
```

When the asset has no violations, the `Violations` section reads:

```
Violations: none
```

When the dependency or impact list exceeds 10 entries, only the first 10 are shown
and a continuation line is appended:

```
  ... (N more)
```

---

## JSON Output (`--format json`)

```json
{
  "asset": {
    "path": "/Game/Characters/BP_Player",
    "type": "Blueprint",
    "file_size": 1468006,
    "fs_path": "Content/Characters/BP_Player.uasset"
  },
  "dependencies": {
    "total": 3,
    "items": [
      { "path": "/Game/Characters/SK_Player", "type": "SkeletalMesh" },
      { "path": "/Game/Characters/T_Player_D", "type": "Texture2D" },
      { "path": "/Game/GameModes/BP_GameMode", "type": "Blueprint" }
    ]
  },
  "referenced_by": {
    "total": 12,
    "items": [
      { "path": "/Game/Levels/L_Main", "type": "World" },
      { "path": "/Game/UI/WBP_HUD", "type": "WidgetBlueprint" }
    ]
  },
  "violations": [
    {
      "rule": "lint/blueprint/event-tick-count",
      "message": "EventTick nodes: 8 (limit: 5)",
      "severity": "error"
    },
    {
      "rule": "lint/naming/blueprint-prefix",
      "message": "Missing prefix 'BP_'",
      "severity": "error"
    }
  ],
  "flags": {
    "referenced": true,
    "in_degree": 12,
    "redirector": false,
    "in_ignore_list": false
  }
}
```

In JSON mode, `dependencies.items` and `referenced_by.items` always contain all
entries (no truncation).

---

## Error Cases

| Condition | Behaviour |
|---|---|
| Asset path not found in DB | Exit `2` with message: `error: asset not found in DB: /Game/...` (hint: run `scan` first) |
| DB missing or unreadable | Exit `2` with message: `error: could not open database` |
| Lint/budget crates return errors | Log warning, omit that section from output, continue |
