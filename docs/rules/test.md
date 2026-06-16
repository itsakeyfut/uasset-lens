# uasset-lens — Testing Standards

## References

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [proptest Book](https://proptest-rs.github.io/proptest/intro.html)
- [Criterion Book](https://bheisler.github.io/criterion.rs/book/)

---

## Philosophy

Test **behavior**, not implementation. A test that breaks only when observable behavior changes
is a good test. A test that breaks when you rename an internal field is not.

---

## Test Naming Convention

All test functions follow the pattern:

```
<feature>_should_<expected_result>
```

```rust
// ✅ Good names — describe what the system should do
fn parse_blueprint_should_extract_import_dependencies()
fn asset_path_should_reject_empty_string()
fn detect_should_return_assets_with_no_incoming_edges()
fn find_impact_should_separate_direct_and_transitive()
fn filter_changed_should_exclude_files_with_unchanged_mtime()

// ❌ Bad names — describe implementation, not behavior
fn test_parse()
fn test_graph()
```

---

## Test Layers

### 1. Unit tests (primary)

Place unit tests in a `#[cfg(test)] mod tests { ... }` block inside the source file.
Each test exercises a single function or method in isolation.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_path_should_reject_path_with_extension() {
        assert!(AssetPath::new("/Game/Chars/BP_Player.uasset").is_err());
    }
}
```

### 2. Integration tests

Integration tests live in `crates/<crate>/tests/` and use **real fixture files** from `tests/fixtures/`.
See `docs/specs/testing.md` for fixture layout and sources.

```rust
// crates/scanner/tests/integration.rs
#[test]
fn scan_files_should_parse_blueprint_fixture() {
    let root = Path::new("tests/fixtures/valid");
    let result = scanner::scan_files(&[root.join("BP_Simple.uasset")], root);
    let meta = &result.assets[0];
    assert_eq!(meta.asset_type, AssetType::Blueprint);
}
```

### 3. Property-based tests (proptest)

Verify that invariants hold for arbitrary inputs. Targets in uasset-lens:

- `AssetPath::new()` — never panics on any string input
- `DependencyGraph::find_impact()` — never panics on any graph
- `find_cycles()` — returned cycles are self-consistent

```toml
[dev-dependencies]
proptest = "1"
```

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn asset_path_new_should_never_panic(s in ".*") {
        let _ = AssetPath::new(s); // must not panic regardless of input
    }
}
```

### 4. Criterion benchmarks

See `docs/rules/perf.md`. Add only to performance-critical paths.

---

## What to Test per Crate

### `shared`

| Target | Test |
|--------|------|
| `AssetPath::new()` | rejects empty string, missing leading slash, paths with extensions; accepts valid paths |
| `AssetPath::from_fs_path()` | rejects files outside content_root; correctly converts to `/Game/` prefix |
| `AssetPath::package_name()` | correctly strips object-name suffix |
| `AssetType` serde | all variants round-trip through serialize/deserialize without loss |

### `scanner`

| Target | Test |
|--------|------|
| `scan_files()` — valid fixtures | correct `AssetType` detected; expected paths in dependencies; nothing in skipped |
| `scan_files()` — bad magic | enters skipped as `ScanError::InvalidMagic` |
| `scan_files()` — truncated | enters skipped as `ScanError::UnexpectedEof` |
| `scan_files()` — `.umap` | returns `AssetType::World` |
| `scan_files()` — ObjectRedirector | type is detected correctly |
| Import filtering | `/Script/` and `/Engine/` references are absent from dependencies |
| Parallel correctness | rayon parallel results match single-threaded results |

### `asset-db`

| Target | Test |
|--------|------|
| `open()` | creates schema on first open; opens existing DB without error |
| `upsert_asset()` + `get_asset()` | round-trip save and retrieve |
| `filter_changed()` | returns new files; excludes unchanged mtime; returns changed mtime |
| `all_edges()` | returns correct edge pairs after upsert |
| `all_assets()` | returns all upserted assets |
| `find_assets()` with `AssetFilter` | type, size, and path-pattern filters work individually and in combination |
| `delete_asset()` | removes the record; dependency edges are CASCADE-deleted |

### `dependency-graph`

| Target | Test |
|--------|------|
| `build()` | isolated nodes (no edges) are present; edges are added correctly |
| `find_cycles()` | returns cycle when present; empty for a DAG; detects two-node mutual reference |
| `find_impact()` — direct only | `direct` is correct; `transitive` is empty |
| `find_impact()` — with transitive | `direct` and `transitive` are correctly separated |
| `find_impact()` — no impact | both lists are empty |
| `in_degree()` | isolated node is 0; referenced node has correct count |
| `nodes()` | returns all nodes including isolated ones |

### `dead-asset-detector`

| Target | Test |
|--------|------|
| `detect()` — isolated nodes present | included in result |
| `detect()` — all nodes referenced | empty list |
| `detect()` — mixed | returns only unreferenced nodes |

### `redirector-analyzer`

| Target | Test |
|--------|------|
| `detect()` — ObjectRedirector present | included in result |
| `detect()` — no Redirector | empty list |
| `detect()` — mixed types | returns only ObjectRedirector paths |

### `cli` (integration)

| Target | Test |
|--------|------|
| `scan` exit codes | clean → `0`; directory not found → `2` |
| `graph --cycles-only` | cycle detected → `1`; clean → `0` |
| `dead-assets` | unreferenced asset found → `1` |
| `--format json` | all commands emit spec-compliant, parseable JSON |
| `impact` JSON | `direct`, `transitive`, and `total` keys are present |

---

## What NOT to Test

- Internal field names and private struct layouts
- Third-party crate internals (`rusqlite`, `petgraph`)
- Platform-specific filesystem behavior (test with abstract paths)
- Macro-generated code
- Trivial getters/setters with no logic

---

## Test Helpers

Define shared test utilities as helper functions inside `#[cfg(test)] mod tests`,
or in `tests/common/` for integration tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph_a_refs_b() -> DependencyGraph {
        DependencyGraph::build(
            vec![
                AssetNode { path: AssetPath::new("/Game/A").unwrap(), asset_type: AssetType::Blueprint },
                AssetNode { path: AssetPath::new("/Game/B").unwrap(), asset_type: AssetType::Blueprint },
            ],
            vec![(
                AssetPath::new("/Game/A").unwrap(),
                AssetPath::new("/Game/B").unwrap(),
            )],
        )
    }

    #[test]
    fn find_impact_should_return_direct_referencing_asset() {
        let graph = make_graph_a_refs_b();
        let result = graph.find_impact(&AssetPath::new("/Game/B").unwrap());
        assert_eq!(result.direct, vec![AssetPath::new("/Game/A").unwrap()]);
        assert!(result.transitive.is_empty());
    }
}
```
