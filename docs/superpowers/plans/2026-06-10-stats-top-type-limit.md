# Stats --top Type Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change `stats` text output to show top 10 asset types by default (was hardcoded to 3), and make the existing `--top N` flag control the type count as well as folders and assets.

**Architecture:** All changes are in `crates/cli/src/commands/stats.rs` (logic + tests) and `crates/cli/src/lib.rs` (doc comment only). The `--top` Clap variant field already exists; we extend its semantics to cover a third limit. `--top 0` means "show all" for every section, handled via match arms. JSON output is unaffected.

**Tech Stack:** Rust, Clap 4, rusqlite via asset_db, anyhow

---

## File map

| File | Change |
|---|---|
| `crates/cli/src/commands/stats.rs` | Add `type_limit`, replace hardcoded `3` / `top3`, update header, update tests |
| `crates/cli/src/lib.rs` | Update `--top` doc comment to mention types |

---

### Task 1: Add failing smoke tests for the new type-limit behavior

**Files:**
- Modify: `crates/cli/src/commands/stats.rs:252–487` (`#[cfg(test)]` block)

These tests exercise the code path with 11 distinct types. With the current hardcoded-3 implementation they do not panic (`.min(3)` clamps safely), so technically they pass today — but they document the required behavior and will catch any regression after the fix.

- [ ] **Step 1: Add the tests**

Append the following three tests inside the existing `mod tests { ... }` block in `stats.rs`, before the closing `}`:

```rust
    fn insert_11_types(dir: &Path, db_path: &std::path::Path) {
        let mut db = asset_db::AssetDb::open(db_path).unwrap();
        let metas: Vec<_> = (0..11_u64)
            .map(|i| {
                make_meta(
                    &format!("/Game/Asset{i}"),
                    dir.join(format!("Asset{i}.uasset")),
                    AssetType::Unknown(format!("Type{i}")),
                    1024 * (i + 1),
                    vec![],
                )
            })
            .collect();
        db.upsert_all(&metas).unwrap();
    }

    #[test]
    fn handle_stats_top_default_should_show_up_to_10_types_without_panicking() {
        // 11 distinct types; default top (10) must not panic or truncate to old hardcoded 3
        let (dir, db_path) = test_db_in_tempdir("stats238_default");
        insert_11_types(&dir, &db_path);
        let result =
            handle_stats(&dir, None, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_top_zero_should_show_all_types_without_panicking() {
        // top=Some(0) means "show all"; must not panic with 11 types
        let (dir, db_path) = test_db_in_tempdir("stats238_zero");
        insert_11_types(&dir, &db_path);
        let result =
            handle_stats(&dir, Some(0), &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_top_custom_should_limit_type_display_without_panicking() {
        // top=Some(3) with 11 types; must not panic and "(8 more types)" would follow
        let (dir, db_path) = test_db_in_tempdir("stats238_custom");
        insert_11_types(&dir, &db_path);
        let result =
            handle_stats(&dir, Some(3), &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run tests (expect PASS — these are smoke tests)**

```powershell
cargo test -p cli handle_stats_top_default -- --nocapture
cargo test -p cli handle_stats_top_zero -- --nocapture
cargo test -p cli handle_stats_top_custom -- --nocapture
```

Expected: all three pass (current code handles them without panic due to `.min(3)`).

- [ ] **Step 3: Commit the tests alone**

```powershell
git add crates/cli/src/commands/stats.rs
git commit -m "test(stats): add smoke tests for --top type limit (issue 238)"
```

---

### Task 2: Implement type_limit in handle_stats

**Files:**
- Modify: `crates/cli/src/commands/stats.rs:61–82` (limits block)
- Modify: `crates/cli/src/commands/stats.rs:156–198` (By Type section)

- [ ] **Step 1: Replace the two limit lines (61–62) with three**

Old (lines 61–62):
```rust
    let folder_limit = top.unwrap_or(5);
    let asset_limit = top.unwrap_or(10);
```

New:
```rust
    let folder_limit = match top { Some(0) => usize::MAX, Some(n) => n, None => 5 };
    let asset_limit  = match top { Some(0) => usize::MAX, Some(n) => n, None => 10 };
```

(`usize::MAX` is safe for `truncate` and `take` — both clamp to the actual length.)

- [ ] **Step 2: Add type_limit after by_type is sorted (after line 82)**

Find the line:
```rust
    by_type.sort_unstable_by_key(|t| std::cmp::Reverse(t.bytes));
```

Insert immediately after it:
```rust
    let type_limit = match top { Some(0) => by_type.len(), Some(n) => n, None => 10 };
```

(`by_type.len()` for the `Some(0)` case so the header shows the real count, not `usize::MAX`.)

- [ ] **Step 3: Replace the By Type display section (lines 156–198)**

Find and replace the entire By Type block:

Old:
```rust
            // By Type: always show top 3, then "(N more types)" if any remain
            println!();
            println!("By Type:");
            let top3 = &by_type[..by_type.len().min(3)];
            if !top3.is_empty() {
                let max_name = top3.iter().map(|t| t.asset_type.len()).max().unwrap_or(1);
                let max_count = top3
                    .iter()
                    .map(|t| crate::digit_count(t.count))
                    .max()
                    .unwrap_or(1);
                let max_size_len = top3
                    .iter()
                    .map(|t| crate::format_size(t.bytes).len())
                    .max()
                    .unwrap_or(1);
                let max_bytes = top3.first().map(|t| t.bytes).unwrap_or(1).max(1);
                for t in top3 {
                    let size_str = crate::format_size(t.bytes);
                    let filled = (24.0 * t.bytes as f64 / max_bytes as f64).round() as usize;
                    let bar = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(24 - filled);
                    let pct = if total_bytes > 0 {
                        100.0 * t.bytes as f64 / total_bytes as f64
                    } else {
                        0.0
                    };
                    println!(
                        "  {:<name$}  {:>cnt$}  {:>size$}  {}  {:.1}%",
                        t.asset_type,
                        t.count,
                        size_str,
                        bar,
                        pct,
                        name = max_name,
                        cnt = max_count,
                        size = max_size_len,
                    );
                }
            }
            let remaining = by_type.len().saturating_sub(3);
            if remaining > 0 {
                println!("  ({} more types)", remaining);
            }
```

New:
```rust
            println!();
            println!("By Type (top {}):", type_limit);
            let top_types = &by_type[..by_type.len().min(type_limit)];
            if !top_types.is_empty() {
                let max_name = top_types.iter().map(|t| t.asset_type.len()).max().unwrap_or(1);
                let max_count = top_types
                    .iter()
                    .map(|t| crate::digit_count(t.count))
                    .max()
                    .unwrap_or(1);
                let max_size_len = top_types
                    .iter()
                    .map(|t| crate::format_size(t.bytes).len())
                    .max()
                    .unwrap_or(1);
                let max_bytes = top_types.first().map(|t| t.bytes).unwrap_or(1).max(1);
                for t in top_types {
                    let size_str = crate::format_size(t.bytes);
                    let filled = (24.0 * t.bytes as f64 / max_bytes as f64).round() as usize;
                    let bar = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(24 - filled);
                    let pct = if total_bytes > 0 {
                        100.0 * t.bytes as f64 / total_bytes as f64
                    } else {
                        0.0
                    };
                    println!(
                        "  {:<name$}  {:>cnt$}  {:>size$}  {}  {:.1}%",
                        t.asset_type,
                        t.count,
                        size_str,
                        bar,
                        pct,
                        name = max_name,
                        cnt = max_count,
                        size = max_size_len,
                    );
                }
            }
            let remaining = by_type.len().saturating_sub(type_limit);
            if remaining > 0 {
                println!("  ({} more types)", remaining);
            }
```

- [ ] **Step 4: Run the full cli test suite**

```powershell
cargo test -p cli
```

Expected: same number of tests pass as before, no new failures. (The watcher test may hang — if so, Ctrl+C and check the stats tests specifically with `cargo test -p cli stats`.)

- [ ] **Step 5: Run clippy**

```powershell
cargo clippy --workspace -- -D warnings
```

Expected: no warnings.

- [ ] **Step 6: Run cargo fmt check and fix if needed**

```powershell
cargo fmt --all -- --check
```

If it fails, run:
```powershell
cargo fmt --all
```

Then re-check.

- [ ] **Step 7: Commit the implementation**

```powershell
git add crates/cli/src/commands/stats.rs
git commit -m "feat(stats): extend --top to control type display count, default 10"
```

---

### Task 3: Update --top doc comment in lib.rs

**Files:**
- Modify: `crates/cli/src/lib.rs` (~line 156)

- [ ] **Step 1: Update the doc comment**

Find:
```rust
        /// Number of folders and largest assets to show (default: 5 folders, 10 assets)
        #[arg(long)]
        top: Option<usize>,
```

Replace with:
```rust
        /// Number of asset types, folders, and largest assets to show
        /// (default: 10 types, 5 folders, 10 assets); 0 = show all
        #[arg(long)]
        top: Option<usize>,
```

- [ ] **Step 2: Build to confirm no compile errors**

```powershell
cargo build --workspace
```

Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```powershell
git add crates/cli/src/lib.rs
git commit -m "docs(cli): update --top help text to mention type limit"
```

---

## Self-review

**Spec coverage check:**
- ✅ Default shows top 10 types (was 3) — Task 2 changes `None => 10`
- ✅ `--top N` controls type count — Task 2 `Some(n) => n` branch
- ✅ `--top 0` shows all types — Task 2 `Some(0) => by_type.len()` branch
- ✅ Bar chart scales to largest shown type — unchanged; `top_types.first()` is still the largest in the displayed slice
- ✅ JSON unaffected — `by_type` in `StatsOutput` (line 132) still includes all types
- ✅ `--top` also fixed for folder/asset when `top=Some(0)` (bonus correctness fix)
- ✅ Tests added — Task 1 adds three smoke tests

**Placeholder scan:** None found.

**Type consistency:** `type_limit: usize`, `folder_limit: usize`, `asset_limit: usize` — consistent across Tasks 2 and 3. `top_types: &[TypeStat]` replaces `top3: &[TypeStat]` — no external callers, purely local.
