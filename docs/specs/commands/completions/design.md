# `completions` Command — Internal Design

## Execution Flow

```
1. Binary entry point (apps/uasset-lens/src/main.rs):
   └── completions subcommand is intercepted BEFORE Clap dispatch
   └── clap_complete::generate(shell, &mut Cli::command(), "uasset-lens", &mut stdout)
   └── script written to stdout, process exits
2. Main CLI dispatch loop is never reached for this command
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Shell completion generation | `clap_complete` (external crate) |
| CLI definition introspection | `uasset-lens-cli` (`Cli::command()`) |
| Entry point interception | `apps/uasset-lens` |

## Why Entry-Point Interception

`completions` is intercepted at the binary level before the standard command dispatch
because it does not require a project directory, a scanned DB, or any of the normal
CLI infrastructure. Running it through the standard dispatch path would require
special-casing the missing `--db` and `--config` arguments.

The interception pattern checks `args[1] == "completions"` before calling
`Cli::parse()`, so `clap_complete` can introspect the full `Cli` struct definition
without any DB or filesystem access.

## Generated Script Scope

`clap_complete` generates completions for all subcommands, arguments, and option
values that are statically declared in the `Cli` struct via `#[derive(Parser)]`.
Dynamic values (e.g. asset paths from the DB) are not included — only the static
CLI surface is covered.

## Installation

The generated script is stateless — it does not embed paths or configurations.
Users source it in their shell startup file:

```bash
# Bash
eval "$(uasset-lens completions bash)"

# PowerShell
uasset-lens completions powershell | Out-String | Invoke-Expression
```
