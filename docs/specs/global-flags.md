# Global Flags — Specification

## Purpose

Global flags are available on **all** `uasset-lens` commands. They must be placed
before the sub-command name on the command line.

```bash
uasset-lens --quiet check ./Project
uasset-lens --no-color scan ./Project
uasset-lens --explain lint/naming/blueprint-prefix
```

---

## `--quiet`

Suppresses all progress indicators and informational output written to stderr. Only
the final result is written to stdout. Errors are still written to stderr.

Exit codes remain meaningful and are the primary output in quiet mode.

```bash
uasset-lens check ./Project --quiet
echo $?  # 0 = pass, 1 = violations found, 2 = execution error
```

| Suppressed | Not suppressed |
|---|---|
| Progress bars | Final summary line (stdout) |
| Informational banners | Error messages (stderr) |
| Per-file scan updates | Exit code |
| Warning-level violations (stderr) | Error-level violations (stdout) |

Use case: scripting and CI pipelines where only the exit code matters and log noise
must be minimized.

---

## `--no-color`

Disables ANSI color escape codes in all output. Applies to both stdout and stderr.

Color is also disabled automatically when:

- stdout is not a TTY (piped or redirected output)
- The `NO_COLOR` environment variable is set to any non-empty value

`NO_COLOR` follows the specification at [https://no-color.org](https://no-color.org).
`--no-color` on the command line takes precedence over any other setting.

```bash
# All equivalent — no ANSI codes in output
uasset-lens check ./Project --no-color
NO_COLOR=1 uasset-lens check ./Project
uasset-lens check ./Project | cat
```

---

## `--explain <RULE>`

Prints a detailed description of a specific lint or budget rule and exits with code `0`.
No scan or check is performed.

```bash
uasset-lens --explain lint/naming/blueprint-prefix
uasset-lens --explain budget/texture2d
```

### Output Format

```
Rule: lint/naming/blueprint-prefix
Category: Naming Convention
Severity: Error (default)

Description:
  Blueprint assets must have a 'BP_' prefix in their name.
  This is a UE5 naming convention for editor discoverability.

Default: enabled
Configurable in: [lint.rules.naming] section of .uasset-lens.toml

Example violation:
  /Game/Characters/Character.uasset (Blueprint) — missing prefix 'BP_'

Example compliant:
  /Game/Characters/BP_Character.uasset
```

If the rule ID is not recognized, the tool exits with code `2` and prints an error:

```
error: unknown rule 'lint/naming/bad-rule'
       Run `uasset-lens --explain` with no argument to list all rules.
```

### Rule ID Format

Rule IDs use the same slash-separated hierarchy as SARIF output:

| Pattern | Example |
|---|---|
| `lint/<category>/<rule-name>` | `lint/naming/blueprint-prefix` |
| `budget/<asset-type>` | `budget/texture2d` |

Run `uasset-lens --explain` with no argument to print a table of all available rule IDs.

---

## Flag Interaction

| Combination | Behavior |
|---|---|
| `--quiet --no-color` | Quiet mode with no ANSI codes |
| `--explain <RULE>` with any sub-command | Sub-command is ignored; explain output is shown and tool exits |
| `--quiet` with `--format json` | JSON result still written to stdout; progress stderr suppressed |
| `--no-color` with `--format json` | No effect (JSON output contains no ANSI codes) |

---

## Exit Codes (global)

These exit codes apply to all commands including global flag invocations.

| Code | Meaning |
|---|---|
| `0` | Success / no violations at or above `--fail-on` threshold |
| `1` | Violations found at or above `--fail-on` threshold |
| `2` | Execution error (I/O failure, invalid arguments, unknown rule) |
