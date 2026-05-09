# `crates/cli` — CLI skeleton, common flags, and path resolution

## Summary

Set up the `cli` crate with the full clap command tree, global flags, DB path resolution,
and content root resolution.
Complete when `uasset-lens --help` displays all commands with their descriptions.

## Design Notes

**Command tree (use clap derive API):**

```
uasset-lens
  --format <text|json>   (default: text)
  --db <PATH>            (override DB location)
  -y / --yes             (skip confirmation prompts)

  scan        <project_dir>  [--full-scan]
  graph       <project_dir>  [--cycles-only]
  dead-assets <project_dir>  [--type <AssetType>]
  impact      <asset_path>
  redirectors <project_dir>
  find        <project_dir>  [--type] [--larger-than] [--smaller-than] [--unreferenced] [--path]
```

Stub each command handler with `todo!()` for now — they will be implemented in subsequent issues.

**Path resolution helpers:**

```rust
fn resolve_db_path(project_dir: &Path, db_override: Option<&Path>) -> PathBuf
// → db_override if Some, else <project_dir>/.uasset-lens/uasset-lens.db

fn resolve_content_root(project_dir: &Path) -> PathBuf
// → <project_dir>/Content/ if that directory exists, else <project_dir>
```

**`cli::run()` signature:**

```rust
pub fn run() -> i32   // called by main.rs; returns exit code
```

Parse args, dispatch to handler, convert `anyhow::Error` to stderr output + exit code 2.

## Requirements

- [ ] Define top-level `Cli` struct with `--format`, `--db`, `-y` flags and `Commands` subcommand enum
- [ ] Add all 6 subcommands (scan, graph, dead-assets, impact, redirectors, find) with stub handlers
- [ ] Define `FormatKind` enum (`Text`, `Json`) used by `--format`
- [ ] Implement `resolve_db_path(project_dir, db_override) -> PathBuf`
- [ ] Implement `resolve_content_root(project_dir) -> PathBuf`
- [ ] Implement `cli::run() -> i32` entry point with error-to-stderr conversion
- [ ] `uasset-lens --help` lists all 6 subcommands without panicking

## Related

- Depends on: #1 (workspace, bin crate not yet created)
- Next: #13 — scan command core
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 5, `docs/rules/cli-output.md`
