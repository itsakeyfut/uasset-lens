# uasset-lens Git Hooks

A git pre-commit hook that runs `uasset-lens check` on staged `.uasset` / `.umap`
files and blocks the commit when an asset violates a lint or budget rule.

The hook runs:

```
uasset-lens check <PROJECT_DIR> --only lint,budget --skip-scan --fail-on error
```

`--skip-scan` reuses the existing scan index instead of re-indexing — keep it
up to date by running `uasset-lens scan` as part of your asset-save workflow.
See [`docs/specs/integrations/pre-commit.md`](../specs/integrations/pre-commit.md)
for the full specification.

## Prerequisites

- `uasset-lens` is installed and on your `PATH`.
- The scan index is current (the hook does not scan; it reads the existing index).

## Manual installation

```bash
cp docs/hooks/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The script must be executable. On Windows (Git Bash / WSL), the `chmod` step is
required when the file was created outside a Unix environment.

Because `.git/hooks/` is not committed to version control, each developer must run
the install command after cloning.

## Project directory

The script defaults to the current directory (`.`) as the UE project root (the
folder containing `Content/`). Override it without editing the script:

```bash
export UASSET_LENS_PROJECT_DIR=./MyProject
```

Or edit the `PROJECT_DIR` line in your installed copy of the hook.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | All checks passed — commit proceeds |
| `1` | One or more violations at or above `--fail-on error` — commit blocked |
| `2` | Execution error (`uasset-lens` not found, I/O failure) — commit blocked |

## pre-commit framework integration

As an alternative to the manual script, the hook can be managed by the
[pre-commit](https://pre-commit.com) framework. This repository ships a
[`.pre-commit-hooks.yaml`](../../.pre-commit-hooks.yaml) at its root, so consuming
repositories can reference the hook by URL.

Add this to the consuming repository's `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/itsakeyfut/uasset-lens
    rev: v0.2.0
    hooks:
      - id: uasset-lens-check
        args: [./MyProject] # your UE project directory
```

The check flags (`--only lint,budget --skip-scan --fail-on error`) are baked into
the hook's `entry`, so the consuming repo only supplies the project directory via
`args`. `pass_filenames: false` stops pre-commit from forwarding individual file
paths — `uasset-lens check` expects a project directory, not a file list.
