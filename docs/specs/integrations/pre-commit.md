# Git Pre-Commit Hook — Specification

## Purpose

A git pre-commit hook runs `uasset-lens check` on staged `.uasset` and `.umap` files
before every commit. Non-zero exit code from the tool blocks the commit.

---

## Hook Behavior

1. Lists staged `.uasset` / `.umap` files using `git diff --cached`.
2. If no matching files are staged, exits `0` immediately (no-op).
3. Runs `uasset-lens check` with `--skip-scan` to avoid re-indexing.
4. Propagates the exit code to git — non-zero blocks the commit.

`--skip-scan` is used because the scan index should already be up-to-date from the
asset save workflow. Teams should run `uasset-lens scan` as part of saving assets
(e.g., via the UE5 editor save hook or a CI step). Running a full scan inside a
pre-commit hook is too slow for interactive use.

---

## Hook Script

```bash
#!/usr/bin/env bash
set -euo pipefail

# UE project directory (the folder containing Content/). Override with UASSET_LENS_PROJECT_DIR.
PROJECT_DIR="${UASSET_LENS_PROJECT_DIR:-.}"

# Collect staged .uasset / .umap files
STAGED=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(uasset|umap)$' || true)

if [ -z "$STAGED" ]; then
  exit 0
fi

if ! command -v uasset-lens >/dev/null 2>&1; then
  echo "uasset-lens: command not found — install it to enable asset checks." >&2
  exit 2
fi

echo "uasset-lens: checking $(echo "$STAGED" | wc -l | tr -d ' ') staged asset(s)..."

# A non-zero exit propagates via set -e and blocks the commit.
uasset-lens check "$PROJECT_DIR" \
  --only lint,budget \
  --skip-scan \
  --fail-on error
```

`PROJECT_DIR` defaults to the current directory and can be overridden with the
`UASSET_LENS_PROJECT_DIR` environment variable. The `command -v` guard returns
exit `2` (per the table below) with a clear message when `uasset-lens` is not
installed, instead of bash's bare `command not found`.

---

## Installation

### Manual

```bash
cp docs/hooks/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The script must be executable. On Windows (Git Bash / WSL), the `chmod` step is
required when the file was created outside of a Unix environment.

### Shared via repository

Store the script at `docs/hooks/pre-commit.sh` and document the install step in your
project README. Because `.git/hooks/` is not committed to version control, each
developer must run the install command after cloning.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | All checks passed — commit proceeds |
| `1` | One or more violations at or above `--fail-on` threshold — commit blocked |
| `2` | Execution error (tool not found, I/O failure) — commit blocked |

---

## pre-commit Framework Integration

As an alternative to a manual shell script, the hook can be managed by the
[pre-commit](https://pre-commit.com) framework.

### `.pre-commit-hooks.yaml` (in this repository)

```yaml
- id: uasset-lens-check
  name: uasset-lens asset check
  description: Run lint and budget checks on staged UE5 assets before commit
  entry: uasset-lens check --only lint,budget --skip-scan --fail-on error
  language: system
  files: \.(uasset|umap)$
  pass_filenames: false
```

The check flags are baked into `entry` rather than `args`. A consuming repo's
`args` *replaces* the hook's default `args`, so putting the flags in `args` would
drop them as soon as a consumer sets `args` to point at their project directory.

### `.pre-commit-config.yaml` (in consuming repositories)

```yaml
repos:
  - repo: https://github.com/itsakeyfut/uasset-lens
    rev: v0.2.0
    hooks:
      - id: uasset-lens-check
        args: [./MyProject]
```

The consumer supplies only the project directory via `args`; it is appended to the
baked-in `entry`. The `pass_filenames: false` setting prevents pre-commit from
forwarding individual file paths to the tool — `uasset-lens check` expects a project
directory, not a file list.

---

## Flags Reference

| Flag | Description |
|---|---|
| `--only lint,budget` | Run only lint and budget checks (skip dead-asset, duplicate, etc.) |
| `--skip-scan` | Use the existing scan index without re-scanning |
| `--fail-on error` | Block commit only on errors; warnings are reported but allowed |
