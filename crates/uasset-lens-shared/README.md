# uasset-lens-shared

> Core domain types for Unreal Engine 5 asset analysis.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-shared.svg)](https://crates.io/crates/uasset-lens-shared)
[![docs.rs](https://docs.rs/uasset-lens-shared/badge.svg)](https://docs.rs/uasset-lens-shared)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Foundational types shared across the uasset-lens toolchain: `AssetPath` (a validated
UE game path such as `/Game/Characters/BP_Player`), `AssetType`, and `FPackageVersion`.
Has no heavy dependencies, so it is cheap to depend on from any UE5 tooling.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
provides the core domain types every other crate is built on. Depends on: nothing internal.
Used by: every other `uasset-lens-*` crate.

## Usage

```rust
use uasset_lens_shared::{AssetPath, AssetType};

let path = AssetPath::new("/Game/Characters/BP_Player")?;
assert_eq!(path.as_str(), "/Game/Characters/BP_Player");

assert_eq!(AssetType::Texture2D.to_string(), "Texture2D");
# Ok::<(), uasset_lens_shared::AssetPathError>(())
```

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
