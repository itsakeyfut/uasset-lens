# `config` Command — Specification

## Purpose

Subcommand group for inspecting and validating the project configuration. The initial
subcommand is `validate`, which reads `.uasset-lens.toml`, checks TOML syntax, validates
all field types and value constraints, and reports errors. Unknown fields produce warnings
but do not cause a non-zero exit.

```bash
uasset-lens config validate ./Project
uasset-lens config validate --config ./custom.toml
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Config is valid (warnings are allowed) |
| `1` | One or more validation errors found |
| `2` | I/O error or TOML parse error (file unreadable or structurally malformed) |

---

## `validate` Subcommand

### No Config Found

When no config file is present at the resolved path:

```
No config file found at .uasset-lens.toml (using defaults).
```

Exits `0`. A missing config is not an error — the tool uses built-in defaults.

---

### Text Output — Valid

```
$ uasset-lens config validate ./Project

.uasset-lens.toml is valid.
```

With warnings (unknown fields):

```
$ uasset-lens config validate ./Project

.uasset-lens.toml is valid.

  1 warning:
    line 23: [scan] — unknown field 'exclude_glob' (did you mean 'exclude_patterns'?)
```

---

### Text Output — Invalid

```
$ uasset-lens config validate ./Project

.uasset-lens.toml: 3 errors found

  line 12: [lint.rules.naming] — missing required field 'prefix'
  line 18: [budget.limits.Texture2D] — value '0' must be > 0
  line 23: [scan] — unknown field 'exclude_glob' (did you mean 'exclude_patterns'?)
```

Errors and warnings are listed in file order (ascending line number).

---

### Text Output — TOML Parse Error

When the file is not valid TOML (structurally broken):

```
$ uasset-lens config validate ./Project

error: .uasset-lens.toml failed to parse (exit 2)

  line 7: expected `.`, `=`
```

---

## JSON Output (`--format json`)

Valid:

```json
{
  "valid": true,
  "path": ".uasset-lens.toml",
  "errors": [],
  "warnings": []
}
```

Invalid:

```json
{
  "valid": false,
  "path": ".uasset-lens.toml",
  "errors": [
    { "line": 12, "section": "lint.rules.naming", "message": "missing required field 'prefix'" },
    { "line": 18, "section": "budget.limits.Texture2D", "message": "value '0' must be > 0" }
  ],
  "warnings": [
    { "line": 23, "section": "scan", "message": "unknown field 'exclude_glob' (did you mean 'exclude_patterns'?)" }
  ]
}
```

No config found:

```json
{
  "valid": true,
  "path": ".uasset-lens.toml",
  "present": false,
  "errors": [],
  "warnings": []
}
```

---

## Validation Scope

`validate` checks:

- TOML syntax (handled by the TOML parser before field-level validation)
- Required fields per section are present
- Numeric fields satisfy minimum and maximum constraints (e.g., budget values must be > 0)
- Enum fields hold one of the allowed string values
- Unknown top-level and nested fields (reported as warnings with typo suggestions where
  possible)

`validate` does not check:

- Whether referenced paths exist on disk
- Whether the DB schema is compatible with this config
