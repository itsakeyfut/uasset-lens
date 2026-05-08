# `crates/shared` — AssetType enum and FPackageVersion struct

## Summary

Define the `AssetType` enum covering all known UE5 asset class names and the `FPackageVersion`
struct for tracking package file format versions.
Complete when `cargo test -p shared` passes for these two types.

## Design Notes

**AssetType variants:**

```rust
Blueprint, BlueprintInterface, AnimBlueprint, UserWidget,
StaticMesh, SkeletalMesh, Texture2D,
Material, MaterialInstance, MaterialFunction,
SoundWave, SoundCue, AnimSequence, AnimMontage,
DataTable, DataAsset, World, ObjectRedirector,
Unknown(String),
```

Required derives: `Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize`

**FPackageVersion fields:**

```rust
pub struct FPackageVersion {
    pub legacy_version: i32,
    pub file_version_ue4: u32,
    pub file_version_ue5: u32,
}
```

`is_ue5()` returns `true` when `legacy_version == -8 && file_version_ue5 > 0`.

Both types live in `crates/shared/src/` and are re-exported from `lib.rs`.

## Requirements

- [ ] Define `AssetType` enum with all 18 named variants plus `Unknown(String)`
- [ ] Add `Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize` derives to `AssetType`
- [ ] Implement `Display` for `AssetType` (named variants return variant name; `Unknown` returns the inner string)
- [ ] Define `FPackageVersion` struct with `legacy_version: i32`, `file_version_ue4: u32`, `file_version_ue5: u32`
- [ ] Implement `is_ue5() -> bool` on `FPackageVersion`
- [ ] Re-export both types from `crates/shared/src/lib.rs`
- [ ] Unit tests: `AssetType` serde round-trip, `Unknown(String)` serde, `is_ue5()` true/false cases

## Related

- Depends on: #1 (workspace setup)
- Next: #3 — AssetPath newtype
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 2
