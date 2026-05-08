# `crates/scanner` — ImportTable parser

## Summary

Implement `parser/import.rs` to extract hard-reference dependency paths from the import
table, returning only `/Game/` asset paths as `AssetPath` values.
Complete when the known imports from `BP_Simple.uasset` are returned correctly.

## Design Notes

**`FObjectImport` layout (all fields are indices or offsets, read sequentially):**

```
ClassPackage  i32  — name table index (package containing the class, e.g. "/Script/Engine")
ClassName     i32  — name table index (class name, e.g. "Blueprint")
OuterIndex    i32  — index into object table: negative = import table, positive = export table, 0 = none
ObjectName    i32  — name table index (the object/asset name)
PackageIndex  i32  — UE5: additional package reference (read and discard for now)
```

**Reconstructing the game path:**

Walk the `OuterIndex` chain to build path components from inner-most to outer-most.
The outermost entry (OuterIndex == 0) whose ObjectName resolves to a path starting with
`/Game/` is the package path. This is the value to convert to `AssetPath`.

**Filtering rules (applied before `AssetPath` construction):**
- Discard paths starting with `/Script/` (engine code packages)
- Discard paths starting with `/Engine/` (engine content)
- Keep only paths starting with `/Game/`

**Function signature:**

```rust
pub fn parse_import_table(
    data: &[u8],
    offset: u64,
    count: usize,
    name_table: &[String],
) -> Result<Vec<AssetPath>, ScanError>
```

> **Note**: The exact path reconstruction logic requires inspecting the outer index chain.
> Refer to how UE resolves `FObjectImport::GetPathName()` in the engine source.

## Requirements

- [ ] Implement `parse_import_table(data, offset, count, name_table) -> Result<Vec<AssetPath>, ScanError>`
- [ ] Read each `FObjectImport` in order: ClassPackage, ClassName, OuterIndex, ObjectName, PackageIndex
- [ ] Walk OuterIndex chain to reconstruct the full object path string
- [ ] Apply filter: discard `/Script/` and `/Engine/` paths; keep only `/Game/` paths
- [ ] Convert retained paths to `AssetPath` (use internal constructor to bypass `new()` extension check)
- [ ] Unit test: `BP_Simple.uasset` fixture → expected list of `/Game/` dependency paths
- [ ] Unit test: paths with `/Script/Engine` and `/Engine/` prefixes are excluded from result

## Related

- Depends on: #6 (name_table parser), #3 (AssetPath)
- Next: #8 — ExportTable parser + scan_files()
