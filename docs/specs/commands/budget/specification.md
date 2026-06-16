# `budget` Command — Specification

## Purpose

Report assets that exceed the per-type file size budgets configured in
`.uasset-lens.toml`. Exits `1` if any violations are found, making it usable as a CI gate.

```bash
uasset-lens budget ./Project
```

Budgets are configured in the `[budget]` section of `.uasset-lens.toml`.
If no budget configuration exists, the command reports nothing and exits `0`.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No budget violations found |
| `1` | One or more assets exceed their type's budget |
| `2` | Execution error |

---

## Configuration

```toml
# .uasset-lens.toml
[budget]
Texture2D.max_file_size = "4MB"
SoundWave.max_file_size = "2MB"
StaticMesh.max_file_size = "10MB"
```

See `docs/specs/config.md` for the full budget configuration reference.

---

## Text Output

```
$ uasset-lens budget ./Project

Budget Violations:

  Texture2D (limit: 4.0 MB)
    /Game/Environments/T_Terrain_D    32.0 MB  (+28.0 MB)
    /Game/Characters/T_Player_HD_D     8.4 MB   (+4.4 MB)

  SoundWave (limit: 2.0 MB)
    /Game/Audio/SFX_Ambience_Long     18.2 MB  (+16.2 MB)

Budget violations: 3 assets across 2 types
```

No violations:

```
Budget: all assets within configured limits
```

---

## GitHub Actions Output (`--format github-actions`)

Each violation is emitted as an inline PR annotation:

```
::error file=Content/Environments/T_Terrain_D.uasset,title=budget.Texture2D::32.0 MB exceeds limit 4.0 MB (+28.0 MB)
::error file=Content/Audio/SFX_Ambience_Long.uasset,title=budget.SoundWave::18.2 MB exceeds limit 2.0 MB (+16.2 MB)
```

---

## JSON Output (`--format json`)

```json
[
  {
    "path": "/Game/Environments/T_Terrain_D",
    "type": "Texture2D",
    "file_size": 33554432,
    "limit": 4194304,
    "excess": 29360128
  },
  {
    "path": "/Game/Audio/SFX_Ambience_Long",
    "type": "SoundWave",
    "file_size": 19083673,
    "limit": 2097152,
    "excess": 16986521
  }
]
```
