# `init` Command — Internal Design

## Execution Flow

```
1. Resolve config_path = project_dir / ".uasset-lens.toml"
2. If config_path exists and --force not given:
   └── print error to stderr
   └── exit 1
3. Determine preset:
   └── if --preset given: use it directly
   └── elif -y / --yes: use "indie" default
   └── else: run interactive prompt (stdin/stderr)
       a. ask "Project scale? [indie/mid/aaa] (default: indie):"
       b. ask "Content root name? [Content] (default: Content):"
       c. ask "Write .uasset-lens.toml? [Y/n]:"
          └── if "n": abort without writing, exit 0
4. Load preset template string for chosen preset    [cli]
5. If content root differs from "Content": patch the template string
6. fs::write(config_path, template)                [cli]
   └── on I/O error → exit 2
7. Print success message to stdout
8. Print .gitignore tip to stdout
9. exit 0
```

---

## Crate Responsibilities

| Step | Crate |
|---|---|
| Interactive prompt (stdin/stderr) | `uasset-lens-cli` |
| Preset template strings | `uasset-lens-cli` (init/presets.rs) |
| File write | `uasset-lens-cli` |
| JSON output formatting | `uasset-lens-cli` |

No library crates are involved. Config templates are embedded as string constants; no
external template engine is used.

---

## Preset Templates

Each preset is a static `&str` holding the complete TOML content. Templates are stored
in `crates/uasset-lens-cli/src/commands/init/presets.rs` and selected by a match:

```rust
fn preset_template(preset: Preset) -> &'static str {
    match preset {
        Preset::Indie => include_str!("templates/indie.toml"),
        Preset::Mid   => include_str!("templates/mid.toml"),
        Preset::Aaa   => include_str!("templates/aaa.toml"),
    }
}
```

The template files are checked into the repository under
`crates/uasset-lens-cli/src/commands/init/templates/`.

---

## Content Root Patching

When the user provides a non-default content root name (anything other than `"Content"`),
the command performs a single string replacement on the template before writing:

```
"content_root = \"Content\""  →  "content_root = \"<user_value>\""
```

This avoids a full TOML parse-modify-serialize round-trip for a single field substitution.

---

## Interactive Prompt Rules

- All prompts are written to **stderr** so that `--format json` output on stdout is not
  mixed with interactive text.
- If stdin is not a TTY (e.g., piped input or CI), the command behaves as if `-y` were
  given and uses all defaults. It does not hang waiting for input.
- The final confirmation (`[Y/n]`) defaults to `Y`; an empty input confirms.

---

## `.gitignore` Tip

The tip is printed unconditionally after a successful write, regardless of whether
`.uasset-lens/` is already present in `.gitignore`. Checking `.gitignore` is out of scope
for this command.
