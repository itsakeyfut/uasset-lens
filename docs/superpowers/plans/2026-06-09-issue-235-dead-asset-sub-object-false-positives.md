# Issue #235: Dead Asset Sub-Object False Positive Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Exclude UE5 sub-object types that can never be referenced by other assets from dead-asset detection, with a `--include-all-types` escape hatch on the `dead-assets` command.

**Architecture:** Add `excluded_type_names: &[&str]` to `dead_asset_detector::detect()` and a public `DEFAULT_EXCLUDED_TYPES` constant. The CLI passes this constant by default; `--include-all-types` passes `&[]` to opt in. `clean` and `check` always pass the constant (no opt-in flag — safety by default).

**Tech Stack:** Rust, `shared::AssetType` (`Display` impl used for string comparison), `dependency_graph::DependencyGraph`, Clap 4.

---

## File Map

| File | Change |
|---|---|
| `crates/dead-asset-detector/src/lib.rs` | Add `DEFAULT_EXCLUDED_TYPES` constant; change `detect()` to accept `excluded_type_names: &[&str]` |
| `crates/cli/src/commands/dead_assets.rs` | Add `include_all_types: bool` param; update `detect()` call |
| `crates/cli/src/commands/clean.rs` | Update `detect()` call to pass `DEFAULT_EXCLUDED_TYPES` |
| `crates/cli/src/commands/check.rs` | Update `detect()` call to pass `DEFAULT_EXCLUDED_TYPES` |
| `crates/cli/src/lib.rs` | Add `--include-all-types` flag to `DeadAssets` variant; wire dispatch |

---

## Background

`dead_asset_detector::detect()` returns all nodes with `in_degree == 0`. UE5 generates certain sub-object files (MetaData, BillboardComponent, etc.) that can never appear in another asset's import table by design — their in_degree is always 0. These are false positives.

`AssetType` has named variants for known types (Blueprint, StaticMesh…) and `Unknown(String)` for everything else. Sub-object types like `MetaData` appear as `Unknown("MetaData")`. `AssetType::Unknown("MetaData").to_string()` returns `"MetaData"`, so a `&[&str]` denylist comparison works for all cases.

Currently there are **3 call sites** for `detect()`:
- `commands/dead_assets.rs` — the `dead-assets` command (gets the opt-in flag)
- `commands/clean.rs` — the `clean` command (always uses the denylist)
- `commands/check.rs` — the `check` command (always uses the denylist)

---

### Task 1: Extend `detect()` with exclusion list

**Files:**
- Modify: `crates/dead-asset-detector/src/lib.rs`

- [ ] **Step 1: Write 3 failing tests**

Add inside the existing `#[cfg(test)]` block in `crates/dead-asset-detector/src/lib.rs`. The existing `node()` helper creates `AssetType::Blueprint` nodes; add a typed helper for unknown types:

```rust
fn typed_node(path: &str, type_name: &str) -> AssetNode {
    AssetNode {
        path: AssetPath::new(path).unwrap(),
        asset_type: AssetType::Unknown(type_name.to_owned()),
    }
}

#[test]
fn detect_should_exclude_sub_object_type_when_type_is_in_excluded_list() {
    let graph = DependencyGraph::build(
        vec![
            node("/Game/BP_Character"),
            typed_node("/Game/Meta", "MetaData"),
        ],
        vec![],
        &[] as &[&str],
    );
    let result: Vec<_> = detect(&graph, &["MetaData"])
        .into_iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    assert_eq!(result, vec!["/Game/BP_Character"]);
}

#[test]
fn detect_should_include_sub_object_types_when_excluded_list_is_empty() {
    let graph = DependencyGraph::build(
        vec![
            node("/Game/BP_Character"),
            typed_node("/Game/Meta", "MetaData"),
        ],
        vec![],
        &[] as &[&str],
    );
    let mut result: Vec<_> = detect(&graph, &[])
        .into_iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    result.sort();
    assert_eq!(result, vec!["/Game/BP_Character", "/Game/Meta"]);
}

#[test]
fn detect_should_not_exclude_blueprint_type_when_only_metadata_is_excluded() {
    let graph = DependencyGraph::build(
        vec![
            node("/Game/BP_Character"),
            typed_node("/Game/Meta", "MetaData"),
        ],
        vec![],
        &[] as &[&str],
    );
    let result: Vec<_> = detect(&graph, &["MetaData"])
        .into_iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    // Blueprint is not in the denylist, so it still appears
    assert_eq!(result, vec!["/Game/BP_Character"]);
}
```

- [ ] **Step 2: Run to confirm compile failure**

```
cargo test -p dead-asset-detector 2>&1 | head -20
```

Expected: compile error — `detect()` called with 2 arguments but takes 1.

- [ ] **Step 3: Implement the constant and updated `detect()`**

Replace the current `pub fn detect(...)` function and add the constant above it in `crates/dead-asset-detector/src/lib.rs`:

```rust
use dependency_graph::DependencyGraph;
use shared::{AssetPath, is_ofpa_path};

/// UE5 generates these sub-object types internally. They can never appear
/// in another asset's import table by design, so in_degree == 0 is structural
/// rather than a sign of an orphaned asset.
pub const DEFAULT_EXCLUDED_TYPES: &[&str] = &[
    "MetaData",
    "AssetImportData",
    "BillboardComponent",
    "ActorFolder",
    "ArrowComponent",
    "BlueprintGeneratedClass",
    "AnimCurveMetaData",
    "BodySetup",
];

pub fn detect(graph: &DependencyGraph, excluded_type_names: &[&str]) -> Vec<AssetPath> {
    graph
        .nodes()
        .filter(|node| graph.in_degree(&node.path) == 0)
        .filter(|node| !is_ofpa_path(node.path.as_str()))
        .filter(|node| {
            if excluded_type_names.is_empty() {
                return true;
            }
            let t = node.asset_type.to_string();
            !excluded_type_names.contains(&t.as_str())
        })
        .map(|node| node.path.clone()) // clone required: AssetPath is not Copy
        .collect()
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p dead-asset-detector 2>&1
```

Expected: `test result: ok. 8 passed` (5 existing + 3 new).

- [ ] **Step 5: Commit**

```
git add crates/dead-asset-detector/src/lib.rs
git commit -m "feat(dead-assets): add DEFAULT_EXCLUDED_TYPES and excluded_type_names param to detect()"
```

---

### Task 2: Wire exclusion into all CLI call sites and add `--include-all-types`

**Files:**
- Modify: `crates/cli/src/commands/dead_assets.rs`
- Modify: `crates/cli/src/commands/clean.rs`
- Modify: `crates/cli/src/commands/check.rs`
- Modify: `crates/cli/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)]` in `crates/cli/src/commands/dead_assets.rs`:

```rust
#[test]
fn handle_dead_assets_should_exclude_sub_object_types_by_default() {
    let (dir, db_path) = test_db_in_tempdir("dead235_default");
    {
        let mut db = asset_db::AssetDb::open(&db_path).unwrap();
        db.upsert_all(&[
            make_meta(
                "/Game/BP_Character",
                dir.join("BP_Character.uasset"),
                AssetType::Blueprint,
                4096,
                vec![],
            ),
            make_meta(
                "/Game/Meta",
                dir.join("Meta.uasset"),
                AssetType::Unknown("MetaData".to_owned()),
                512,
                vec![],
            ),
        ])
        .unwrap();
    }
    let result = handle_dead_assets(
        &dir,
        None,
        false,
        None,
        &[],
        None,
        false, // include_all_types = false → MetaData excluded
        &db_path,
        &Default::default(),
        &FormatKind::Text,
    )
    .unwrap();
    assert_eq!(result, 1, "MetaData is excluded; only Blueprint counts as dead");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn handle_dead_assets_should_include_sub_object_types_when_include_all_types_is_true() {
    let (dir, db_path) = test_db_in_tempdir("dead235_all");
    {
        let mut db = asset_db::AssetDb::open(&db_path).unwrap();
        db.upsert_all(&[
            make_meta(
                "/Game/BP_Character",
                dir.join("BP_Character.uasset"),
                AssetType::Blueprint,
                4096,
                vec![],
            ),
            make_meta(
                "/Game/Meta",
                dir.join("Meta.uasset"),
                AssetType::Unknown("MetaData".to_owned()),
                512,
                vec![],
            ),
        ])
        .unwrap();
    }
    let result = handle_dead_assets(
        &dir,
        None,
        false,
        None,
        &[],
        None,
        true, // include_all_types = true → MetaData included
        &db_path,
        &Default::default(),
        &FormatKind::Text,
    )
    .unwrap();
    assert_eq!(result, 1, "both are dead, but result is still 1 (dead assets found)");
    let _ = std::fs::remove_dir_all(&dir);
}
```

Add to `#[cfg(test)]` in `crates/cli/src/commands/clean.rs`:

```rust
#[test]
fn handle_clean_should_not_target_metadata_type_assets() {
    let (dir, db_path) = test_db_in_tempdir("clean235_meta");
    {
        let mut db = asset_db::AssetDb::open(&db_path).unwrap();
        db.upsert_all(&[make_meta(
            "/Game/Meta",
            dir.join("Meta.uasset"),
            AssetType::Unknown("MetaData".to_owned()),
            512,
            vec![],
        )])
        .unwrap();
    }
    // dry_run = true so no file deletion; MetaData should not appear as a target
    let result = handle_clean(
        &dir,
        true,
        true,
        None,
        &[],
        None,
        &db_path,
        &Default::default(),
        &FormatKind::Text,
    )
    .unwrap();
    assert_eq!(result, 0, "MetaData is excluded; no clean targets → exit 0");
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run to confirm compile failure**

```
cargo test -p cli dead235 2>&1 | head -30
```

Expected: compile errors — `detect()` and `handle_dead_assets()` called with wrong argument counts.

- [ ] **Step 3: Update `handle_dead_assets()` in `dead_assets.rs`**

Change the function signature — add `include_all_types: bool` after `group`:

```rust
#[allow(clippy::too_many_arguments)]
pub fn handle_dead_assets(
    _project_dir: &Path,
    asset_type_filter: Option<&str>,
    sort_by_size: bool,
    min_size: Option<u64>,
    exclude_patterns: &[String],
    group: Option<&GroupMode>,
    include_all_types: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;

    let excluded = if include_all_types {
        &[] as &[&str]
    } else {
        dead_asset_detector::DEFAULT_EXCLUDED_TYPES
    };
    let dead_paths = dead_asset_detector::detect(&graph, excluded);
    // ... rest of function unchanged
```

- [ ] **Step 4: Update `detect()` call in `clean.rs`**

Find line 58: `let dead_paths = dead_asset_detector::detect(&graph);`

Replace with:

```rust
let dead_paths = dead_asset_detector::detect(&graph, dead_asset_detector::DEFAULT_EXCLUDED_TYPES);
```

- [ ] **Step 5: Update `detect()` call in `check.rs`**

Find the line: `let dead = dead_asset_detector::detect(g);`

Replace with:

```rust
let dead = dead_asset_detector::detect(g, dead_asset_detector::DEFAULT_EXCLUDED_TYPES);
```

- [ ] **Step 6: Update `lib.rs` — add flag to `DeadAssets` variant**

In the `DeadAssets` variant (after `group: Option<GroupMode>`), add:

```rust
/// Include sub-object types excluded by default (MetaData, BillboardComponent, etc.)
#[arg(long)]
include_all_types: bool,
```

In the `Commands::DeadAssets` dispatch arm, destructure `include_all_types` and pass it:

```rust
Commands::DeadAssets {
    project_dir,
    asset_type,
    sort_by_size,
    min_size,
    exclude_patterns,
    group,
    include_all_types,
} => {
    let db_path = resolve_db_path(project_dir, cli.db.as_deref());
    commands::dead_assets::handle_dead_assets(
        project_dir,
        asset_type.as_deref(),
        *sort_by_size,
        *min_size,
        exclude_patterns,
        group.as_ref(),
        *include_all_types,
        &db_path,
        &cfg,
        &cli.format,
    )
}
```

- [ ] **Step 7: Fix all existing tests that call `handle_dead_assets()`**

All existing tests in `dead_assets.rs` call `handle_dead_assets()` with the old signature (9 args). Add `false` as the `include_all_types` argument (7th positional, after `group`).

Each call changes from:
```rust
handle_dead_assets(
    project_dir,
    asset_type_filter,
    sort_by_size,
    min_size,
    exclude_patterns,
    group,
    db_path,           // ← was 7th
    cfg,
    format,
)
```

To:
```rust
handle_dead_assets(
    project_dir,
    asset_type_filter,
    sort_by_size,
    min_size,
    exclude_patterns,
    group,
    false,             // include_all_types
    db_path,
    cfg,
    format,
)
```

There are 18 existing test call sites — update all of them.

- [ ] **Step 8: Run tests**

```
cargo test -p dead-asset-detector -p cli 2>&1 | grep -E "^test result"
```

Expected: all pass. `dead-asset-detector`: 8 total; `cli`: all existing + 3 new.

- [ ] **Step 9: Run full quality check**

```
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

All must pass.

- [ ] **Step 10: Commit**

```
git add crates/cli/src/commands/dead_assets.rs crates/cli/src/commands/clean.rs crates/cli/src/commands/check.rs crates/cli/src/lib.rs
git commit -m "feat(cli): add --include-all-types to dead-assets; exclude sub-object types by default in dead-assets, clean, and check"
```
