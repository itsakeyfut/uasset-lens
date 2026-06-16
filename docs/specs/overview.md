# uasset-lens Overview

## Overview

A Rust-based static analysis tool for assets and Blueprints in Unreal Engine 5 projects.
Designed to run in CI pipelines without opening the Unreal Editor.

This tool addresses the following pain points that emerge as Unreal projects grow large:

- Opaque asset dependency chains
- Blueprint sprawl
- Accumulation of unused assets
- Circular dependencies
- Difficulty diffing assets with Git
- Increasing asset management overhead
- Bloated package / cook sizes
- Hard-to-review changes in team development

The goal is to be

> "Clippy for Unreal Assets" — place asset quality gates in CI without opening the editor

---

## Target Users

- Unreal Engine developers
- Indie game developers
- Technical Artists
- Tools Programmers
- Gameplay Programmers
- Teams working on large-scale Unreal projects

---

## Problems to Solve

### Asset Explosion

As Unreal projects grow, the following problems appear:

- No clear picture of what references what
- Cannot safely delete unused assets
- Blueprint circular references
- Blueprint complexity explosion
- Redirector hell
- Fear of renaming assets
- Increasing build / cook times
- Package size bloat

### Blueprint Black Box Problem

Because Blueprints are GUI-based:

- Cannot be grepped
- Difficult to diff
- Difficult to review
- Difficult to statically analyze

As a result, Blueprint maintainability drops sharply once they grow large.

---

## Software Concept

### Concept

"Clippy for Unreal Assets" — a CLI-first static analyzer that integrates into CI pipelines
and enforces asset quality gates without requiring the Unreal Editor.

### Value Proposition

- Asset health analysis (dead assets, circular deps, duplicates)
- Blueprint static analysis (complexity, EventTick abuse, Cast chains)
- Dependency graph with soft reference tracking
- Naming convention and file size budget enforcement
- Git-friendly: `--diff-from baseline` for PR regression detection
- GitHub Actions annotation output for inline PR review

### Design Priorities

- Fast (1,000 assets in under 5 seconds)
- Parallel analysis (rayon)
- CLI first — no GUI
- CI integration (exit codes, `--format github-actions`)
- Git friendly
- Cross-platform
- Large project friendly (up to 100,000 assets)

### Non-Functional Requirements

| Requirement | Target |
|-------------|--------|
| Memory usage | ≤ 100 MB (must coexist with UE5 + VS in the background) |
| Scan speed | 1,000 assets in under 5 seconds (full scan, parallel) |
| Maximum scale | Up to 100,000 assets |

#### Design Constraints

- Never expand all assets into memory at once (streaming / chunk processing)
- Delegate large datasets to SQLite; keep only the needed portion of the graph in memory
- Meet the speed target through parallel scanning (rayon)

---

## Core Philosophy

### Not aiming to replace the engine

Out of scope:

- A custom game engine
- An Unreal replacement
- A general-purpose engine

Direction:

> Augment Unreal Engine development

---

## CLI First — No Desktop App

uasset-lens is CLI-only. There is no desktop GUI and no plans for one.

Reasons:

- CI integration requires CLI (not GUI)
- OSS adoption is driven by `cargo install` + GitHub Actions, not app stores
- Automation and scripting require CLI
- Large project analysis (100k assets) is impractical in a GUI
- Static HTML reports replace any visualization need

---

## Competitive Landscape

### Limitations of existing Unreal tools

Unreal's built-in tools:

- Weak asset visualization
- Weak Blueprint diff
- Hard to manage large-scale assets
- Heavy GUI dependency

uasset-lens fills that gap.

---

## Ultimate Vision

> Make UE asset quality gates as standard as code linting in CI.

Any Unreal Engine 5 project should be able to add asset quality gates to its CI pipeline
in under 5 minutes with a single command:

```yaml
- run: uasset-lens check ./Project --format github-actions
```

The tool becomes the de facto answer to "how do I enforce asset quality in CI without
opening the Unreal Editor?"
