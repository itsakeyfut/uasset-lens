# uasset-lens-watcher

> Debounced filesystem watch sessions for Unreal Engine 5 asset directories.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-watcher.svg)](https://crates.io/crates/uasset-lens-watcher)
[![docs.rs](https://docs.rs/uasset-lens-watcher/badge.svg)](https://docs.rs/uasset-lens-watcher)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A thin, debounced wrapper over `notify` that coalesces filesystem events into batches keyed
by change kind (created / changed / deleted), so an asset-import burst surfaces as one event
instead of dozens. Built for incremental re-scan loops over a project's `Content/` tree.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
powers the CLI `watch` command. Depends on: `notify`. Used by: the uasset-lens CLI.

## Usage

```rust,no_run
use std::path::Path;
use uasset_lens_watcher::Watcher;

let watcher = Watcher::new(Path::new("MyProject/Content"))?;

while let Some(batch) = watcher.next_batch() {
    for event in &batch {
        println!("{:?}: {} path(s)", event.kind, event.paths.len());
    }
}
# Ok::<(), uasset_lens_watcher::WatcherError>(())
```

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
