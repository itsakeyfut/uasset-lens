# `blueprint` Command — Specification

## Purpose

Show a complexity ranking of all Blueprint assets in the project. Helps identify
Blueprints that have grown too large and are candidates for refactoring.

```bash
uasset-lens blueprint ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Always (unless execution error) |
| `2` | Execution error |

---

## Metrics Reported

| Metric | Description |
|---|---|
| Node count | Total number of nodes in the Blueprint graph |
| EventTick count | Number of Event Tick nodes (high = performance risk) |
| Cast count | Number of Cast nodes (high = tight coupling) |
| Dependency depth | Longest transitive dependency chain from this Blueprint |

---

## Text Output

```
$ uasset-lens blueprint ./Project

Blueprint Complexity Ranking

Rank  Asset                                    Nodes  EventTick  Casts  DepDepth
   1  /Game/Characters/BP_Player               324       8         23       7
   2  /Game/UI/WBP_MainMenu                    201       3         12       4
   3  /Game/GameModes/BP_GameMode              187       2          8       5
   4  /Game/Characters/BP_Enemy                156       5         15       3
   5  /Game/Weapons/BP_WeaponSystem            143       1          6       4
...

Total: 47 Blueprints analyzed
```

Ranked by total node count, descending.

---

## JSON Output (`--format json`)

```json
[
  {
    "path": "/Game/Characters/BP_Player",
    "type": "Blueprint",
    "node_count": 324,
    "event_tick_count": 8,
    "cast_count": 23,
    "dependency_depth": 7
  },
  {
    "path": "/Game/UI/WBP_MainMenu",
    "type": "UserWidget",
    "node_count": 201,
    "event_tick_count": 3,
    "cast_count": 12,
    "dependency_depth": 4
  }
]
```
