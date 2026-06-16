# uasset-lens Overview

## Overview

A Rust-based static analysis, visualization, and audit tool for assets and Blueprints in
Unreal Engine projects.

This tool addresses the following pain points that emerge as Unreal projects grow large:

- Opaque asset dependency chains
- Blueprint sprawl
- Accumulation of unused assets
- Circular dependencies
- Difficulty diffing assets with Git
- Increasing asset management overhead
- Bloated package / cook sizes
- Hard-to-review changes in team development

Rather than being just another Asset Viewer, the goal is to serve as an

> Unreal Project Observability Platform

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

"Clippy for Unreal Assets"

### Value Proposition

- Asset visualization
- Asset health analysis
- Blueprint static analysis
- Dependency graph
- Lint
- Git-friendly analysis

### Design Priorities

- Fast
- Parallel analysis
- CLI first
- CI integration
- Git friendly
- Cross-platform
- Large project friendly

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

## CLI First Approach

CLI takes priority over GUI in the early stages.

Reasons:

- Faster to implement
- CI integration
- OSS friendly
- Automation friendly
- Large project friendly

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

The final goal of this project is to

> Make Unreal projects observable

Specifically, to visualize:

- Assets
- Blueprints
- Dependencies
- Complexity
- Project health

and improve the maintainability of large Unreal projects.
