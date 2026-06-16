# `.uasset-lens-ignore` — Specification

## Purpose

`.uasset-lens-ignore` is a `.gitignore`-style file placed in the project root that
excludes asset paths from **all** analysis operations: scan indexing, dead-asset
detection, lint, budget, check, find, and stats.

Location: `<project_dir>/.uasset-lens-ignore`

---

## Syntax

One pattern per line.

| Syntax | Description |
|---|---|
| `# comment` | Lines starting with `#` are ignored |
| *(blank line)* | Blank lines are ignored |
| `Content/Dev/` | Prefix match — excludes all assets under this directory |
| `Content/**/Test*.uasset` | Glob pattern — standard glob syntax (`*`, `**`, `?`) |
| `!Content/Dev/BP_KeepThis.uasset` | Negation — re-includes a previously excluded path |

Patterns are matched against the canonical asset path relative to the project root
(e.g., `Content/Characters/BP_Hero.uasset`). Matching is case-insensitive on
Windows, case-sensitive on Linux/macOS.

---

## Example File

```
# Developer sandbox directories
Content/Dev/
Content/Developers/

# Test fixtures
Content/**/Test*.uasset
Content/QA/

# Re-include specific kept asset
!Content/Dev/BP_SharedLibrary.uasset
```

---

## Interaction with `.uasset-lens.toml`

| Source | Supported Syntax | Notes |
|---|---|---|
| `.uasset-lens-ignore` | Prefix, glob, negation | Evaluated after TOML exclusions |
| `scan.exclude_paths` in TOML | Prefix match only | Evaluated first |

The two sources are **additive**: a path excluded by either is excluded from all
operations. `.uasset-lens-ignore` does not override `scan.exclude_paths`; it extends
it. Negation patterns (`!`) can only re-include paths that were excluded by an earlier
pattern in `.uasset-lens-ignore` — they do not override TOML `exclude_paths`.

---

## Evaluation Order

1. TOML `scan.exclude_paths` prefix rules are applied first.
2. `.uasset-lens-ignore` patterns are evaluated top-to-bottom.
3. A path is included if the last matching pattern is a negation; excluded otherwise.
4. A path not matched by any pattern is included.

---

## Git Integration

`.uasset-lens-ignore` itself **should be committed** to version control so the whole
team shares the same exclusions. The tool's local data directory (`.uasset-lens/`)
should be added to `.gitignore` instead:

```
# .gitignore
.uasset-lens/
```

Rationale: the ignore file is a project-wide policy; the `.uasset-lens/` directory
holds local scan state (SQLite DB, baselines) that is machine-specific.

---

## Scope

Exclusions defined in `.uasset-lens-ignore` apply to every sub-command. Excluded
assets are invisible to the tool — they are not indexed, not reported as dead, not
linted, and not counted in stats.

To verify which paths are being excluded, run:

```bash
uasset-lens scan ./Project --dry-run
```

Excluded paths are shown in the dry-run output with an `[ignored]` marker.
