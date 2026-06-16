# `graph` Command — Specification

## Purpose

Display a summary of the asset dependency graph and detect circular dependencies.

```bash
uasset-lens graph ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Graph displayed (or `--cycles-only` with no cycles found) |
| `1` | `--cycles-only` was specified and at least one cycle was detected |
| `2` | Execution error |

Without `--cycles-only`, the command always exits `0` regardless of whether cycles exist.

---

## Text Output

```
$ uasset-lens graph ./Project

Dependency Graph Summary
  Total assets   : 1,024
  Total edges    : 4,231
  Circular deps  : 2 cycles detected

Cycles:
  [1] BP_Player → BP_Enemy → BP_GameMode → BP_Player
  [2] M_Rock → MF_Shared → M_Rock
```

With no cycles:

```
Dependency Graph Summary
  Total assets   : 1,024
  Total edges    : 4,231
  Circular deps  : none
```

---

## Cycles-Only Mode (`--cycles-only`)

Suppresses the summary header and prints only the cycle list. Exits `1` if any cycle
is found, making it suitable as a CI gate.

```
$ uasset-lens graph ./Project --cycles-only

[1] BP_Player → BP_Enemy → BP_GameMode → BP_Player
[2] M_Rock → MF_Shared → M_Rock

2 cycles detected
```

```bash
# CI usage: fail the build on any circular dependency
uasset-lens graph ./Project --cycles-only
```

---

## Long Cycle Display

For cycles with more than a few nodes, intermediate nodes are collapsed by default:

```
[1] BP_Player → BP_Enemy → ... (3 more) → BP_Player
```

Use `--full-cycles` to print all nodes in the cycle:

```
[1] BP_Player → BP_Enemy → BP_Weapon → BP_AmmoType → BP_Inventory → BP_Player
```

---

## JSON Output (`--format json`)

```json
{
  "total_assets": 1024,
  "total_edges": 4231,
  "cycles": [
    ["/Game/BP_Player", "/Game/BP_Enemy", "/Game/BP_GameMode", "/Game/BP_Player"],
    ["/Game/M_Rock", "/Game/MF_Shared", "/Game/M_Rock"]
  ]
}
```
