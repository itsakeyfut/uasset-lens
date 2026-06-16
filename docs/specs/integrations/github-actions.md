# GitHub Actions Integration — Specification

## Purpose

`itsakeyfut/uasset-lens-action` is a GitHub Actions action that downloads the
`uasset-lens` binary, scans a UE5 project, and runs all configured checks. It is
intended for use in CI pipelines to gate pull requests on asset quality.

The action lives in a **separate repository** (`itsakeyfut/uasset-lens-action`). This
spec defines the contract (inputs, outputs, behavior) that the action must implement.

---

## Inputs

| Input | Required | Default | Description |
|---|---|---|---|
| `project-dir` | yes | — | Path to the UE5 project directory (must contain `Content/`) |
| `format` | no | `github-actions` | Output format; `github-actions` emits workflow commands for inline annotations |
| `fail-on` | no | `error` | Minimum severity that causes a non-zero exit: `error` or `warning` |
| `checks` | no | *(all)* | Comma-separated list of checks to run (e.g., `lint,budget`); omit for all |

---

## Outputs

| Output | Description |
|---|---|
| `check-result` | `pass` or `fail` |
| `violations-count` | Total number of violations found across all checks |

---

## Behavior

1. Determines the runner OS and downloads the matching `uasset-lens` release binary
   from `itsakeyfut/uasset-lens/releases`.
2. Runs `uasset-lens scan <project-dir>` to build the asset index.
3. Runs `uasset-lens check <project-dir> --format <format> --fail-on <fail-on>`.
4. Sets `check-result` and `violations-count` outputs.
5. Exits with the check command's exit code.

When `format: github-actions` is used, violation messages are emitted as
`::error file=<path>::<message>` or `::warning file=<path>::<message>` workflow
commands so that GitHub displays them as inline PR annotations.

---

## Platform Support

| Platform | Status |
|---|---|
| `ubuntu-latest` (linux-x64) | Supported (initial release) |
| `windows-latest` | Planned (later release) |
| `macos-latest` | Not planned |

---

## Example Workflow

```yaml
name: Asset Quality Gate

on:
  pull_request:
    paths:
      - '**.uasset'
      - '**.umap'

jobs:
  asset-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          lfs: true

      - name: Check UE5 assets
        uses: itsakeyfut/uasset-lens-action@v1
        with:
          project-dir: ./MyProject
          fail-on: error
```

---

## Advanced Usage

```yaml
- name: Check UE5 assets (lint and budget only)
  uses: itsakeyfut/uasset-lens-action@v1
  with:
    project-dir: ./MyProject
    checks: lint,budget
    fail-on: warning
    format: github-actions

- name: Print violation count
  run: echo "Violations found: ${{ steps.asset-check.outputs.violations-count }}"
```

---

## Versioning

The action is versioned independently from the `uasset-lens` CLI. Use a pinned tag
(e.g., `@v1.2.0`) in production workflows to avoid unexpected behavior from new
releases. The `@v1` floating tag tracks the latest `v1.x` release.

---

## Caching

To reduce download time on repeated runs, the action stores the downloaded binary in
the GitHub Actions cache keyed by the tool version and runner OS. No additional
configuration is required.
