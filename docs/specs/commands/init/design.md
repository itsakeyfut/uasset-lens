# `init` Command — Internal Design

## Execution Flow

```
1. Resolve config_path = project_dir / ".uasset-lens.toml"
2. If config_path exists and --force not given:
   └── print error to stderr
   └── exit 1
3. Determine preset:
   └── if --preset given: use it directly
   └── elif -y / --yes or stdin is not a TTY: use "indie" default
   └── else: run interactive prompt (stdin/stderr)
       a. ask "Project scale? [indie/mid/aaa] (default: indie):"
       b. ask "Write .uasset-lens.toml? [Y/n]:"
          └── if "n": abort without writing, exit 0
4. Load preset template string for chosen preset    [cli]
5. fs::write(config_path, template)                 [cli]
   └── on I/O error → exit 2
6. Print success message to stdout
7. Print .gitignore tip to stdout
8. exit 0
```

---

## Crate Responsibilities

| Step | Crate |
|---|---|
| Interactive prompt (stdin/stderr) | `apps/cli` |
| Preset template strings | `apps/cli` (commands/init.rs) |
| File write | `apps/cli` |
| JSON output formatting | `apps/cli` |

No library crates are involved. Config templates are embedded as string constants; no
external template engine is used.

---

## Preset Templates

Each preset is a static `&str` holding the complete TOML content, selected by a match:

```rust
fn template(self) -> &'static str {
    match self {
        Preset::Indie => include_str!("init/templates/indie.toml"),
        Preset::Mid   => include_str!("init/templates/mid.toml"),
        Preset::Aaa   => include_str!("init/templates/aaa.toml"),
    }
}
```

The template files are checked into the repository under
`apps/cli/src/commands/init/templates/`. They use only the implemented config schema (e.g.
budgets are `<Type>.max_size = <bytes>`); there is no content-root field to patch, so the
template is written verbatim.

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
