# `.uasset-lens.toml` Configuration Spec

## Overview

`.uasset-lens.toml` is placed in the project root and checked into git for team sharing.
If absent, all defaults apply (see below).

The configuration follows an **eslint-style model**: defaults are strict and designed for
large projects. Smaller teams relax rules as needed.

```
MyUEProject/
  ├─ Content/
  ├─ .uasset-lens/
  │   ├─ uasset-lens.db        # auto-generated, add to .gitignore
  │   └─ baseline.json         # commit to git for regression detection
  └─ .uasset-lens.toml         # commit to git for team sharing
```

---

## Full Schema

```toml
# .uasset-lens.toml

# ---------------------------------------------------------------------------
# [scan] — controls what gets scanned
# ---------------------------------------------------------------------------
[scan]
# Paths to exclude from scanning (relative to content root, prefix match).
exclude_paths = [
    "Content/Dev/",
    "Content/Test/",
    "Content/Developers/",
]

# ---------------------------------------------------------------------------
# [rules] — enable/disable top-level checks
#
# Severity values:
#   "error" → causes `check` to exit 1 (blocks CI)
#   "warn"  → shown in output, does not block CI
#   "off"   → disabled entirely
# ---------------------------------------------------------------------------
[rules]
dead-assets       = "warn"    # Unreferenced assets ("warn" because WIP assets are common)
circular-deps     = "error"   # Circular dependency cycles
duplicate-assets  = "warn"    # Same-name / same-content duplicates
redirectors       = "warn"    # Unresolved ObjectRedirector assets

# ---------------------------------------------------------------------------
# [lint] — naming conventions and Blueprint complexity
# ---------------------------------------------------------------------------
[lint]

# Naming conventions
naming.enabled              = true
naming.severity             = "warn"
naming.texture_prefix       = "T_"
naming.material_prefix      = "M_"
naming.material_function_prefix = "MF_"
naming.static_mesh_prefix   = "SM_"
naming.skeletal_mesh_prefix = "SK_"
naming.blueprint_prefix     = "BP_"
naming.widget_prefix        = "WBP_"
naming.anim_bp_prefix       = "ABP_"
naming.sound_prefix         = "SFX_"

# Blueprint complexity thresholds (strict defaults — large project values)
blueprint.enabled                = true
blueprint.severity               = "error"
blueprint.event_tick_limit       = 1      # max EventTick nodes per Blueprint
blueprint.cast_limit             = 10     # max Cast nodes per Blueprint
blueprint.node_limit             = 200    # max total nodes per Blueprint
blueprint.dependency_depth_limit = 20     # max transitive dependency depth (UE5 PostProcess ABPs reach ~16)

# Per-asset-type override of dependency_depth_limit. Keys are asset type names
# (e.g. Blueprint, AnimBlueprint). Types not listed fall back to the global limit.
[lint.blueprint.depth_by_type]
AnimBlueprint = 20

# ---------------------------------------------------------------------------
# [budget] — per-type file size limits
# ---------------------------------------------------------------------------
[budget]
enabled = true

Texture2D.max_file_size    = "4MB"
Texture2D.severity         = "error"

SoundWave.max_file_size    = "2MB"
SoundWave.severity         = "warn"

StaticMesh.max_file_size   = "10MB"
StaticMesh.severity        = "warn"

SkeletalMesh.max_file_size = "15MB"
SkeletalMesh.severity      = "warn"

# ---------------------------------------------------------------------------
# [check] — check command settings
# ---------------------------------------------------------------------------
[check]
# Default path for --save-baseline / --diff-from.
baseline_path = ".uasset-lens/baseline.json"
```

---

## Defaults Table

When `.uasset-lens.toml` is absent, these defaults apply:

| Key | Default |
|---|---|
| `scan.exclude_paths` | `[]` |
| `rules.dead-assets` | `"warn"` |
| `rules.circular-deps` | `"error"` |
| `rules.duplicate-assets` | `"warn"` |
| `rules.redirectors` | `"warn"` |
| `lint.naming.enabled` | `true` |
| `lint.naming.severity` | `"warn"` |
| `lint.blueprint.enabled` | `true` |
| `lint.blueprint.severity` | `"error"` |
| `lint.blueprint.event_tick_limit` | `1` |
| `lint.blueprint.cast_limit` | `10` |
| `lint.blueprint.node_limit` | `200` |
| `lint.blueprint.dependency_depth_limit` | `20` |
| `budget.enabled` | `true` |
| `budget.Texture2D.max_file_size` | `"4MB"` |
| `budget.Texture2D.severity` | `"error"` |
| `check.baseline_path` | `".uasset-lens/baseline.json"` |

---

## File Size Format

File sizes accept both byte integers and human-readable strings:

```toml
Texture2D.max_file_size = "4MB"     # 4 * 1024 * 1024 bytes
Texture2D.max_file_size = "512KB"   # 512 * 1024 bytes
Texture2D.max_file_size = 4194304   # raw bytes
```

Supported suffixes: `B`, `KB`, `MB`, `GB` (case-insensitive, powers of 1024).

---

## Example Configurations

### Indie / solo developer (relaxed)

```toml
[scan]
exclude_paths = ["Content/Dev/"]

[rules]
dead-assets      = "off"    # WIP assets are common in solo projects
circular-deps    = "warn"   # Warn instead of fail
duplicate-assets = "off"

[lint]
naming.enabled    = false   # Flexible naming during prototyping
blueprint.enabled = false   # No complexity enforcement

[budget]
enabled = false             # No budget enforcement
```

### Mid-size studio (balanced)

```toml
[rules]
dead-assets    = "error"
circular-deps  = "error"

[lint]
blueprint.event_tick_limit = 10   # Slightly more permissive than default
blueprint.node_limit       = 300

[budget]
Texture2D.max_file_size    = "8MB"   # Larger budget than default
```

### Large studio (stricter than default)

```toml
[rules]
dead-assets    = "error"

[lint]
blueprint.event_tick_limit = 0     # Forbid EventTick entirely (default already allows only 1)
blueprint.cast_limit       = 5
blueprint.node_limit       = 100

[budget]
Texture2D.max_file_size    = "2MB"   # Half the default
Texture2D.severity         = "error"
SoundWave.max_file_size    = "1MB"
SoundWave.severity         = "error"
StaticMesh.max_file_size   = "5MB"
StaticMesh.severity        = "error"
```

---

## Glob Pattern Support in `scan.exclude_paths`

In addition to prefix-string matching, `scan.exclude_paths` supports glob patterns
starting with `v0.2.0`:

```toml
[scan]
exclude_paths = [
    "Content/Dev/",                    # prefix match (existing)
    "Content/**/Test*.uasset",         # glob pattern (new)
    "Content/Developers/*/MyStuff/**", # nested glob
]
```

Patterns are matched against the relative path from the content root (not the game path).

For more expressive ignore rules (negation, per-directory granularity), see
`docs/specs/integrations/ignore-file.md`.

---

## Config Inheritance

Starting from `v0.4.0`, `.uasset-lens.toml` supports an `extends` key to inherit
from a parent config file. This is useful for monorepos or multi-project setups where
a base config is shared.

```toml
# MySubProject/.uasset-lens.toml
extends = "../.uasset-lens.base.toml"

# Override only what differs in this subproject
[budget]
Texture2D.max_file_size = "2MB"   # Stricter than the base config
```

Inheritance rules:
- `extends` is resolved relative to the config file's directory.
- Only one level of inheritance is supported (no chaining).
- Keys defined in the child config override the parent. All other keys inherit from parent.
- If the parent file is missing, `uasset-lens` exits with error `2`.

---

## Config Loading Rules

1. `uasset-lens` looks for `.uasset-lens.toml` by walking up from `<project_dir>`.
2. The first file found is used.
3. If the config contains `extends`, the parent config is loaded and merged (child overrides parent).
4. Missing keys fall back to defaults — partial configs are valid.
5. Unknown keys produce a warning on stderr but do not cause an error.
