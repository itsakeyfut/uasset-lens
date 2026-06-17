# uasset-lens-dependency-graph

> In-memory dependency graph for Unreal Engine 5 assets — impact, dead-asset, and redirector analysis.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-dependency-graph.svg)](https://crates.io/crates/uasset-lens-dependency-graph)
[![docs.rs](https://docs.rs/uasset-lens-dependency-graph/badge.svg)](https://docs.rs/uasset-lens-dependency-graph)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Builds a directed dependency graph of UE5 assets (on `petgraph`) and answers structural
questions: what breaks if an asset is renamed or deleted (`find_impact`), which assets are
unreferenced (dead-asset detection), and which are leftover `ObjectRedirector`s.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
is the dependency-analysis layer. Depends on: `uasset-lens-shared`. Used by: the uasset-lens CLI.

## Usage

```rust
use uasset_lens_dependency_graph::{AssetNode, DependencyGraph};
use uasset_lens_shared::{AssetPath, AssetType};

let player = AssetPath::new("/Game/BP_Player")?;
let mesh = AssetPath::new("/Game/SK_Player")?;

let nodes = vec![
    AssetNode { path: player.clone(), asset_type: AssetType::Blueprint },
    AssetNode { path: mesh.clone(), asset_type: AssetType::SkeletalMesh },
];
let edges = vec![(player.clone(), mesh.clone())]; // BP_Player depends on SK_Player
let no_exclusions: &[&str] = &[];

let graph = DependencyGraph::build(nodes, edges, no_exclusions);

// What would break if SK_Player were deleted?
let impact = graph.find_impact(&mesh);
assert_eq!(impact.direct, vec![player]);
# Ok::<(), uasset_lens_shared::AssetPathError>(())
```

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
