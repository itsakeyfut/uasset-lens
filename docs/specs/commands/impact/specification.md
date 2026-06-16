# `impact` Command — Specification

## Purpose

Show which assets would break if the target asset were deleted or renamed — the reverse
dependency analysis. Answers "is it safe to delete this asset?"

```bash
uasset-lens impact ./Project /Game/Characters/BP_Player
```

Accepts both UE game paths (`/Game/...`) and filesystem paths.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No assets would be impacted |
| `1` | One or more assets reference the target (impact found) |
| `2` | Execution error |

---

## Text Output (flat list, default)

```
$ uasset-lens impact ./Project /Game/Characters/BP_Player

Impact Analysis: /Game/Characters/BP_Player

Direct referencing (3):
  /Game/Levels/L_Main.umap
  /Game/UI/WBP_HUD.uasset
  /Game/GameModes/BP_GameMode.uasset

Transitive referencing (9):
  /Game/Levels/L_Tutorial.umap
  ... (8 more)

Total impact: 12 assets
```

No impact:

```
Impact Analysis: /Game/Unused/T_OldRock

No assets reference this asset. Safe to delete.
```

---

## Tree Mode (`--tree`)

Shows the full propagation tree instead of flat direct/transitive lists.

```bash
uasset-lens impact ./Project /Game/Characters/BP_Player --tree
```

```
Impact Analysis: /Game/Characters/BP_Player

/Game/Levels/L_Main.umap
└─ (no further refs)

/Game/UI/WBP_HUD.uasset
└─ /Game/Levels/L_Main.umap

/Game/GameModes/BP_GameMode.uasset
├─ /Game/Levels/L_Main.umap
└─ /Game/Levels/L_Tutorial.umap
    └─ /Game/Levels/L_World.umap
```

---

## JSON Output (`--format json`)

```json
{
  "target": "/Game/Characters/BP_Player",
  "direct": [
    "/Game/Levels/L_Main",
    "/Game/UI/WBP_HUD",
    "/Game/GameModes/BP_GameMode"
  ],
  "transitive": [
    "/Game/Levels/L_Tutorial"
  ],
  "total": 4
}
```
