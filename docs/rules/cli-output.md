# uasset-lens — CLI Output Rules

## stdout / stderr Separation

| Stream | Purpose |
|--------|---------|
| `stdout` | Command result output (text and JSON) |
| `stderr` | Error messages, warnings, and progress indicators |

This keeps progress noise out of `stdout` when piping output to another tool.

```bash
# ✅ This works correctly
uasset-lens impact ./Project/Content/BP_Player.uasset --format json | jq '.direct[]'
```

---

## stdout / stderr Implementation Rules

### Library crates must not write to stdout/stderr

Do not use `println!` / `eprintln!` in library crates such as `scanner` or `asset-db`.
Only the `cli` crate writes to the terminal. Libraries use `tracing` for logging only.

### Send progress to stderr

```rust
// ✅ Progress to stderr
eprintln!("Scanning {} files...", count);

// ✅ Result to stdout
println!("  {}", asset_path);
```

---

## Text Output Format

### Print the count summary last

```
  /Game/Unused/T_OldTexture
  /Game/Characters/SK_OldEnemy
  ...

  Unreferenced Assets (47 found)
```

### Output even when the count is zero

```
  Dead Assets (0 found)
```

### Send error messages to stderr with an `Error:` prefix

```rust
// ✅
eprintln!("Error: no scan data found.");
eprintln!("Run 'uasset-lens scan <project_dir>' first.");
```

---

## JSON Output (`--format json`)

- Write a **single JSON value** (object or array) to `stdout`
- Do not wrap in an envelope (`{ "ok": true, "data": ... }` style)
- Do not include ANSI color codes in JSON output
- On error, write the error message to `stderr` and exit with code `2` (do not output a JSON error object)
- Format with `serde_json::to_string_pretty`

```rust
// ✅ JSON output
if opts.format == OutputFormat::Json {
    let json = serde_json::to_string_pretty(&result)
        .context("Failed to serialize output")?;
    println!("{json}");
    return Ok(());
}
```

For each command's JSON schema, see the "JSON Output Format" section in `docs/specs/cli-design.md`.

---

## ANSI Color

- Use ANSI color codes only when stdout is a terminal (`IsTerminal`)
- Disable color when the `NO_COLOR` environment variable is set
- Never use color in JSON output
- CI environments typically disable color (`NO_COLOR` or non-TTY)

```rust
// ✅ Terminal check before color
use std::io::IsTerminal;

let use_color = std::io::stdout().is_terminal()
    && std::env::var("NO_COLOR").is_err();
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success — no issues found |
| `1` | Issues detected (dead assets, circular dependencies, impact found, etc.) |
| `2` | Execution error (I/O error, DB not created, parse failure, etc.) |

```rust
// ✅ Exit code via process::exit or via main() return value
fn main() -> ExitCode {
    match run() {
        Ok(IssuesFound::None)  => ExitCode::SUCCESS,       // 0
        Ok(IssuesFound::Some)  => ExitCode::from(1),       // 1
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)                              // 2
        }
    }
}
```

---

## scan Command Deletion Prompt

Always ask the user before deleting DB records for assets that are no longer on disk.
Auto-delete only when the `-y` / `--yes` flag is provided.

```
The following DB records have no corresponding file on disk:
  /Game/Old/BP_Deprecated
  /Game/Temp/M_Test
Remove these 2 records from DB? [y/N]:
```

```rust
// ✅ Prompt unless -y
if !opts.yes {
    eprint!("Remove {} records from DB? [y/N]: ", stale.len());
    let mut ans = String::new();
    std::io::stdin().read_line(&mut ans)?;
    if !ans.trim().eq_ignore_ascii_case("y") {
        return Ok(());
    }
}
```

**Important**: This command only removes DB records. It never touches actual `.uasset` files.
