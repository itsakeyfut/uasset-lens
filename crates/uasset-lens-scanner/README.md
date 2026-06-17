# uasset-lens-scanner

> Hand-written parser for Unreal Engine 5 `.uasset` / `.umap` binary files — no editor required.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-scanner.svg)](https://crates.io/crates/uasset-lens-scanner)
[![docs.rs](https://docs.rs/uasset-lens-scanner/badge.svg)](https://docs.rs/uasset-lens-scanner)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Reads the UE5 package binary format directly — header, name table, imports, exports, and
property streams — to extract each asset's type, hard and soft dependencies, and Blueprint
metrics. Built on `byteorder` + `Cursor`, with per-file error recovery so one corrupt asset
never aborts a batch scan. Supports `.uasset` and `.umap`; IoStore containers are out of scope.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
is the binary parser at the core of the toolchain. Depends on: `uasset-lens-shared`.
Used by: `uasset-lens-asset-db`, `uasset-lens-analysis`.

## Usage

```rust,no_run
use std::path::{Path, PathBuf};
use uasset_lens_scanner::scan_files;

let content_root = Path::new("MyProject/Content");
let files = vec![PathBuf::from("MyProject/Content/Characters/BP_Player.uasset")];

let result = scan_files(&files, content_root);

for asset in &result.assets {
    println!(
        "{} ({}) — {} dependencies",
        asset.asset_path.as_str(),
        asset.asset_type,
        asset.dependencies.len(),
    );
}
for skipped in &result.skipped {
    eprintln!("skipped {}: {}", skipped.file_path.display(), skipped.reason);
}
```

## Feature flags

- `test-support`: exposes `make_meta` and related constructors used by downstream crate tests.

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
