# Development Strategy and Roadmap

## MVP

### What to build first

#### MVP features (as of Phase 2 completion)

- uasset scan
- dependency graph
- circular dependency detection
- dead asset detection
- impact analysis (display the scope of delete / rename impact)

#### Goal of the MVP

Enable the following:

> "Is it safe to delete this asset?"

---

## Six-Phase Structure

| Phase | Theme | Key Commands | MVP |
|-------|-------|-------------|-----|
| 1 | Foundation: Binary Scanner | `scan` | — |
| 2 | Core Analysis | `graph` / `dead-assets` / `impact` | **MVP** |
| 3 | CLI Completion | `redirectors` / `find` + config file | — |
| 4 | Static Analysis | `blueprint` / `lint` / `budget` / `duplicates` | — |
| 5 | Dev Workflow Integration | `watch` + `check` + CI integration | — |
| 6 | Report Generation | `report` (HTML / Markdown) | — |

For detailed tasks and completion criteria per phase, see `docs/roadmap/phase{N}/ROADMAP.md`.

---

## Development Strategy

Do not implement all features at once. Release after Phase 2 and iterate with feedback.

### Implementation Order (within each phase)

#### Phase 1
1. `shared` crate (common type definitions)
2. `scanner` crate (binary parser)
3. `asset-db` crate (SQLite)
4. `cli` crate (`scan` command)

#### Phase 2
1. `dependency-graph` crate
2. `dead-asset-detector` crate
3. `impact-analyzer` crate (stub)
4. `cli` extension (3 commands)

#### Phase 3
1. `redirector-analyzer` crate
2. `asset-db` glob support
3. `cli` extension (2 commands + config file)
4. README / `cargo publish` preparation

#### Phase 4
1. Parser Phase 2 (Export property analysis)
2. `bp-analyzer` crate
3. `duplicate-detector` crate
4. `lint-engine` crate
5. `material-analyzer` / `budget-tracker` crates
6. `cli` extension (4 commands)

#### Phase 5
1. `watcher` crate
2. `git-diff` crate
3. `cli` extension (`watch` command)
4. CI integration documentation

#### Phase 6
1. `report-generator` crate
2. `cli` extension (`report` command, HTML + Markdown output)

---

## Future Extension Ideas

### GitHub PR Integration

Automatically notify on the following during a PR:

- Blueprint complexity increase
- Circular dependency detected
- Asset budget exceeded

### Plugin System

In the future, make Analyzers and Rules pluggable so projects can add their own rules.

---

## Project Name

**uasset-lens**

- CLI binary name: `uasset-lens`
- Cargo package name: `uasset-lens`
- Repository name: `uasset-lens`
