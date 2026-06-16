# `completions` Command — Specification

## Purpose

Generate shell completion scripts for `uasset-lens`. The generated script is printed to
stdout and should be sourced by the shell's startup file.

```bash
uasset-lens completions bash
uasset-lens completions zsh
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Completion script written to stdout |
| `2` | Unknown shell name |

---

## Supported Shells

| Shell | Value |
|---|---|
| Bash | `bash` |
| Zsh | `zsh` |
| Fish | `fish` |
| PowerShell | `powershell` |
| Elvish | `elvish` |

---

## Installation Examples

```bash
# Bash (~/.bashrc)
eval "$(uasset-lens completions bash)"

# Zsh (~/.zshrc)
eval "$(uasset-lens completions zsh)"

# Fish (~/.config/fish/config.fish)
uasset-lens completions fish | source

# PowerShell ($PROFILE)
uasset-lens completions powershell | Out-String | Invoke-Expression
```

---

## Notes

- This command is intercepted at the binary entry point (`apps/uasset-lens/src/main.rs`)
  before the main dispatch loop, so it does not require a scanned project directory.
- The `--format`, `--db`, `--config`, and `--yes` global flags have no effect for this command.
