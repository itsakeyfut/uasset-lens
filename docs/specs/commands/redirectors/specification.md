# `redirectors` Command — Specification

## Purpose

List all `ObjectRedirector` assets in the project. Redirectors accumulate when assets
are renamed or moved in the Unreal Editor without "Fix Up Redirectors" being run, and
can increase cook times and cause confusion in the dependency graph.

```bash
uasset-lens redirectors ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No redirectors found |
| `1` | One or more redirectors detected |
| `2` | Execution error |

---

## Text Output

```
$ uasset-lens redirectors ./Project

/Game/Characters/OldName.uasset
/Game/Meshes/SM_OldRock.uasset
/Game/Materials/M_Deprecated.uasset
/Game/UI/WBP_OldWidget.uasset
/Game/Blueprints/BP_OldEnemy.uasset

Redirectors: 5 found
```

No redirectors:

```
Redirectors: none found
```

---

## JSON Output (`--format json`)

```json
[
  { "path": "/Game/Characters/OldName",       "type": "ObjectRedirector" },
  { "path": "/Game/Meshes/SM_OldRock",        "type": "ObjectRedirector" },
  { "path": "/Game/Materials/M_Deprecated",   "type": "ObjectRedirector" },
  { "path": "/Game/UI/WBP_OldWidget",         "type": "ObjectRedirector" },
  { "path": "/Game/Blueprints/BP_OldEnemy",   "type": "ObjectRedirector" }
]
```

---

## Notes

This command only detects and lists redirectors. It does not resolve redirect targets
or detect broken redirectors (where the target no longer exists).
