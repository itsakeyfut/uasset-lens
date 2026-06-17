# uasset-lens-git-diff

> Diff Unreal Engine 5 assets between Git HEAD and the working tree.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-git-diff.svg)](https://crates.io/crates/uasset-lens-git-diff)
[![docs.rs](https://docs.rs/uasset-lens-git-diff/badge.svg)](https://docs.rs/uasset-lens-git-diff)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Compares an asset's committed version (via `git show HEAD:<path>`) against the copy on disk and
reports the semantic delta: dependencies added/removed, asset-type changes, and Blueprint metric
shifts. Lets CI comment on what actually changed inside a `.uasset` in a pull request.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
provides HEAD-vs-disk asset diffing. Depends on: `uasset-lens-shared`. Used by: the uasset-lens CLI.

## Usage

```rust,no_run
use std::path::Path;
use uasset_lens_git_diff::diff_asset;
use uasset_lens_shared::AssetPath;

let asset = AssetPath::new("/Game/Characters/BP_Player")?;
let project_dir = Path::new("MyProject");
let content_root = Path::new("MyProject/Content");

if let Some(diff) = diff_asset(&asset, project_dir, content_root)? {
    println!(
        "+{} deps, -{} deps",
        diff.deps_added.len(),
        diff.deps_removed.len(),
    );
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
