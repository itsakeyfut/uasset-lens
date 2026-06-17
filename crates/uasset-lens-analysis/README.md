# uasset-lens-analysis

> Static-analysis checks for Unreal Engine 5 assets — lint rules, size budgets, Blueprint and material metrics.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-analysis.svg)](https://crates.io/crates/uasset-lens-analysis)
[![docs.rs](https://docs.rs/uasset-lens-analysis/badge.svg)](https://docs.rs/uasset-lens-analysis)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

The quality-gate layer of uasset-lens: a pluggable `LintEngine` (naming-prefix and
Blueprint-complexity rules out of the box), per-type size budgets, and Blueprint / material
metrics. Designed to run in CI and fail a build when assets drift out of spec.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
holds the lint and budget checks. Depends on: `uasset-lens-shared`, `uasset-lens-scanner`,
`uasset-lens-asset-db`. Used by: the uasset-lens CLI.

## Usage

```rust,no_run
use std::collections::HashMap;
use uasset_lens_analysis::{LintEngine, NamingPrefixRule};
use uasset_lens_asset_db::AssetRecord;

let engine = LintEngine::new(vec![Box::new(NamingPrefixRule::default())]);

let assets: Vec<AssetRecord> = load_indexed_assets(); // e.g. from uasset-lens-asset-db
let metrics = HashMap::new();

for v in engine.run(&assets, &metrics) {
    println!("[{:?}] {} — {}", v.severity, v.asset_path.as_str(), v.message);
}
# fn load_indexed_assets() -> Vec<AssetRecord> { Vec::new() }
```

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
