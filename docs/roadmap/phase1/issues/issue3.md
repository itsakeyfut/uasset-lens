# `crates/shared` — AssetPath newtype

## Summary

Implement the `AssetPath(String)` newtype with path validation, filesystem-to-game-path
conversion, and `package_name()` extraction.
Complete when all `AssetPath` unit tests pass.

## Design Notes

**Validation rules for `new(s: &str)`:**

| Error | Condition |
|---|---|
| `Empty` | input is empty |
| `MissingLeadingSlash` | does not start with `/` |
| `InvalidCharacter` | contains `.` after the last `/` (i.e., has a file extension) |

**`from_fs_path(content_root: &Path, file_path: &Path)`:**
1. Verify `file_path` starts with `content_root` → `Err(NotUnderContentRoot)` if not
2. Strip `content_root` prefix to get relative path
3. Remove file extension (`.uasset` / `.umap`)
4. Prepend `/Game/` and convert path separators to `/`

**`package_name()`:**
Strips the `.ObjectSuffix` from the last path component.
Example: `/Game/Characters/BP_Player.BP_Player` → `/Game/Characters/BP_Player`
(UE asset paths sometimes have this suffix appended.)

**Derives required:** `Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize`

## Requirements

- [ ] Define `AssetPath(String)` newtype
- [ ] Define `AssetPathError` enum: `Empty`, `MissingLeadingSlash`, `InvalidCharacter`, `NotUnderContentRoot`
- [ ] Implement `new(s: &str) -> Result<AssetPath, AssetPathError>` with the 3 validation checks
- [ ] Implement `as_str() -> &str`
- [ ] Implement `from_fs_path(content_root: &Path, file_path: &Path) -> Result<AssetPath, AssetPathError>`
- [ ] Implement `package_name() -> &str` (strips `.ObjectSuffix` from last component if present)
- [ ] Add `Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize` derives
- [ ] Re-export `AssetPath` and `AssetPathError` from `lib.rs`
- [ ] Unit tests: all 3 `new()` error cases, valid path, `from_fs_path` success, `NotUnderContentRoot` error, `package_name` with and without suffix

## Related

- Depends on: #2 (AssetType + FPackageVersion)
- Next: #4 — test fixture files
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 2
