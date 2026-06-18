# `init` Command — Specification

## Purpose

Generate a `.uasset-lens.toml` configuration file in the project root, pre-populated
with values appropriate for the project's scale. Three presets cover the common cases:
`indie` (small), `mid` (medium), and `aaa` (large).

```bash
uasset-lens init ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Config file written successfully |
| `1` | Config file already exists and `--force` was not given |
| `2` | Execution error (I/O failure, permission denied) |

---

## Presets

| Preset | Scale | Texture2D budget | Blueprint budget | Dead-asset check | Lint categories |
|---|---|---|---|---|---|
| `indie` | < 1,000 assets | 8 MB | 2 MB | disabled | naming only |
| `mid` | 1,000–10,000 assets | 6 MB | 1 MB | enabled | all categories |
| `aaa` | > 10,000 assets | 4 MB | 512 KB | enabled | all categories + baseline gating |

The `aaa` preset additionally sets `[check] baseline_path`, which makes `check` auto-diff
against the committed baseline and fail on new error-level violations (a missing baseline
makes `check` exit `2`). Create the baseline once with `check --save-baseline`.

---

## Interactive Flow

When `--preset` is not given, `-y` / `--yes` is not active, and stdin is a TTY, the
command prompts for project scale before writing the file:

```
Project scale? [indie/mid/aaa] (default: indie): mid
Write .uasset-lens.toml? [Y/n]:
```

Pressing Enter at the scale prompt accepts the default (`indie`). Unrecognized input also
falls back to `indie`. Entering `n` at the confirmation prompt aborts without writing any
file (exit `0`). When stdin is not a TTY (CI/piped), the command does not prompt and uses
the `indie` default, as if `-y` were given.

---

## Text Output

```
$ uasset-lens init ./Project

Project scale? [indie/mid/aaa] (default: indie): mid
Write .uasset-lens.toml? [Y/n]:

Wrote .uasset-lens.toml (preset: mid)

Tip: add .uasset-lens/ to your .gitignore to exclude the local database.
```

Non-interactive (with `--preset` and `-y`):

```
$ uasset-lens init ./Project --preset aaa -y

Wrote .uasset-lens.toml (preset: aaa)

Tip: add .uasset-lens/ to your .gitignore to exclude the local database.
```

Existing config (without `--force`):

```
$ uasset-lens init ./Project

error: .uasset-lens.toml already exists. Use --force to overwrite.
```

---

## JSON Output (`--format json`)

```json
{
  "written": true,
  "path": ".uasset-lens.toml",
  "preset": "mid"
}
```

When the file already exists and `--force` was not given:

```json
{
  "written": false,
  "path": ".uasset-lens.toml",
  "error": "config_exists"
}
```

---

## Generated File Structure

The written `.uasset-lens.toml` documents each section with a comment block explaining
the available values. The exact TOML contents are defined by the preset templates in
`apps/cli` and are considered part of the implementation, not this specification.
