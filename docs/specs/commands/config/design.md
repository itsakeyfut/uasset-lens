# `config validate` Command — Internal Design

## Execution Flow

```
1. Resolve config_path:
   └── if --config given: use that path directly
   └── else: project_dir / ".uasset-lens.toml"

2. If config_path does not exist:
   └── print "No config file found at {path} (using defaults)."
   └── exit 0

3. fs::read_to_string(config_path)
   └── on I/O error → print error to stderr → exit 2

4. toml::from_str::<toml::Value>(raw)     [toml crate]
   └── on parse error:
       └── extract line number from toml::de::Error
       └── print structured parse error to stderr
       └── exit 2

5. Config::validate(toml_value)           [cli]
   └── walk the TOML value tree
   └── collect Vec<ConfigError> (errors) and Vec<ConfigWarning> (warnings)
   └── unknown fields → ConfigWarning with optional typo suggestion
   └── constraint violations → ConfigError with line + section + message

6. If errors is non-empty:
   └── print error block (sorted by line number)
   └── exit 1

7. If errors is empty:
   └── print ".uasset-lens.toml is valid."
   └── if warnings non-empty: print warning block
   └── exit 0
```

---

## Crate Responsibilities

| Step | Crate |
|---|---|
| Config file read | `uasset-lens-cli` |
| TOML parse | `toml` crate (already a dependency) |
| Field validation (`Config::validate`) | `uasset-lens-cli` |
| Output formatting (text + JSON) | `uasset-lens-cli` |

No new crates are required.

---

## Validation Data Model

```rust
pub struct ConfigError {
    pub line:    Option<u32>,
    pub section: String,   // TOML path, e.g. "budget.Texture2D"
    pub message: String,   // e.g. "value '0' must be > 0"
}

pub struct ConfigWarning {
    pub line:       Option<u32>,
    pub section:    String,
    pub message:    String,   // e.g. "unknown field 'exclude_glob'"
    pub suggestion: Option<String>,  // e.g. "did you mean 'exclude_patterns'?"
}
```

`Config::validate` returns `(Vec<ConfigError>, Vec<ConfigWarning>)`. The function does
not return a `Result` — all errors are collected and returned together so the user sees
all issues in one pass.

---

## Typo Suggestions

Unknown field names are checked against the set of known field names for that section
using edit distance. A suggestion is offered when the Levenshtein distance to the closest
known field is ≤ 2.

The known field set is a static list per section, maintained alongside the `Config` struct
definition in `uasset-lens-cli`.

---

## TOML Line Numbers

`toml::de::Error` exposes a line/column span for the error location. For field-level
validation errors discovered after a successful TOML parse, line numbers are extracted by
scanning the raw source for the field key. If the line cannot be determined, `line` is
`None` and the error is printed without a line prefix.

---

## Future Subcommands

The `config` subcommand group is designed to accept additional subcommands. Candidates
for future phases:

| Subcommand | Description |
|---|---|
| `show` | Pretty-print the resolved config (defaults merged with file values) |
| `set` | Modify a single field in the config file in-place |
| `diff` | Compare two config files field by field |

These are out of scope for the current phase.
