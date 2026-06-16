# `lint` Command — Specification

## Purpose

Run all configured lint rules and report violations. Exits `1` if any violations are
found, making it usable as a CI quality gate.

```bash
uasset-lens lint ./Project
```

Rules and thresholds are configured in the `[lint]` and `[budget]` sections of
`.uasset-lens.toml`. See `docs/specs/config.md` for the configuration reference.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No lint violations found |
| `1` | One or more violations detected |
| `2` | Execution error |

---

## Rules Checked

| Rule category | What it checks |
|---|---|
| Naming conventions | Asset prefix requirements (T\_, M\_, SM\_, BP\_, etc.) |
| Blueprint complexity | Node count, EventTick count, Cast count, dependency depth |
| Size budgets | Per-type file size limits (from `[budget]` section) |

---

## Text Output

```
$ uasset-lens lint ./Project

Lint Violations (4):

  [naming] /Game/Meshes/Rock.uasset
    StaticMesh missing required prefix 'SM_'

  [naming] /Game/UI/MainMenu.uasset
    UserWidget missing required prefix 'WBP_'

  [blueprint] /Game/Characters/BP_Player.uasset
    EventTick node count (8) exceeds limit (5)

  [budget] /Game/Environments/T_Terrain_D.uasset
    Texture2D file size (32.0 MB) exceeds limit (4.0 MB)

lint failed: 4 violations
```

No violations:

```
lint passed: no violations
```

---

## GitHub Actions Output (`--format github-actions`)

Each violation is emitted as an inline PR annotation:

```
::error file=Content/Meshes/Rock.uasset,title=lint.naming::StaticMesh missing required prefix 'SM_'
::error file=Content/Characters/BP_Player.uasset,title=lint.blueprint::EventTick node count (8) exceeds limit (5)
::error file=Content/Environments/T_Terrain_D.uasset,title=lint.budget::Texture2D 32.0 MB exceeds limit 4.0 MB
```

---

## JSON Output (`--format json`)

```json
[
  {
    "rule": "naming",
    "asset_path": "/Game/Meshes/Rock",
    "file": "Content/Meshes/Rock.uasset",
    "message": "StaticMesh missing required prefix 'SM_'"
  },
  {
    "rule": "blueprint",
    "asset_path": "/Game/Characters/BP_Player",
    "file": "Content/Characters/BP_Player.uasset",
    "message": "EventTick node count (8) exceeds limit (5)"
  }
]
```
