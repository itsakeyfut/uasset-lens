# `rename` Command — Specification

## Purpose

Simulate renaming or moving an asset and show all assets that would need to be
updated. Dry-run only — does not modify any files on disk or in the DB.

```bash
uasset-lens rename ./Project /Game/Characters/BP_OldName /Game/Characters/BP_NewName
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Simulation complete; no assets reference the renamed asset |
| `1` | Simulation complete; one or more assets directly reference the renamed asset (action required in UE Editor) |
| `2` | Execution error (source asset not found in DB, DB error, I/O failure) |

---

## Text Output

```
Rename simulation: /Game/Characters/BP_OldName → /Game/Characters/BP_NewName

Direct references (3) — these assets must be re-saved in UE Editor:
  /Game/Levels/L_Main                (World)
  /Game/UI/WBP_HUD                   (WidgetBlueprint)
  /Game/GameModes/BP_GameMode        (Blueprint)

Transitive impact (9 additional assets may be affected)

Note: This is a simulation only. Use UE Editor's "Fix Up Redirectors" workflow to
      perform the actual rename and update all references.
```

When no assets reference the renamed asset:

```
Rename simulation: /Game/Characters/BP_OldName → /Game/Characters/BP_NewName

No assets reference this asset. Rename is safe with no side effects.

Note: This is a simulation only. Use UE Editor's "Fix Up Redirectors" workflow to
      perform the actual rename and update all references.
```

---

## Definitions

- **Direct references**: assets that directly import or reference the source asset
  (in-degree 1 hop). These must be opened and re-saved in UE Editor after the rename.
- **Transitive impact**: assets that do not directly reference the source asset but
  transitively depend on it through the reference chain. Count only; not listed.

---

## JSON Output (`--format json`)

```json
{
  "source": "/Game/Characters/BP_OldName",
  "destination": "/Game/Characters/BP_NewName",
  "direct_references": {
    "total": 3,
    "items": [
      { "path": "/Game/Levels/L_Main", "type": "World" },
      { "path": "/Game/UI/WBP_HUD", "type": "WidgetBlueprint" },
      { "path": "/Game/GameModes/BP_GameMode", "type": "Blueprint" }
    ]
  },
  "transitive_impact_count": 9,
  "simulation_only": true
}
```

---

## Error Cases

| Condition | Behaviour |
|---|---|
| Source asset not found in DB | Exit `2`: `error: asset not found in DB: /Game/...` (hint: run `scan` first) |
| Destination path already exists in DB | Print warning line: `warning: /Game/.../BP_NewName already exists in DB`; continue simulation |
| DB not found | Exit `2` with error message |
