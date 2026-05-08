# Add test fixture `.uasset` files

## Summary

Add real UE5 `.uasset` / `.umap` files to `tests/fixtures/valid/` and synthetic error
binaries to `tests/fixtures/invalid/`.
These fixtures are required by all scanner integration tests.

## Design Notes

**Valid fixtures** — obtain from an actual UE5.1+ project and copy as-is:

| File | Expected AssetType | Notes |
|---|---|---|
| `BP_Simple.uasset` | Blueprint | must have at least one `/Game/` import |
| `T_Rock.uasset` | Texture2D | |
| `SM_Cube.uasset` | StaticMesh | |
| `M_Basic.uasset` | Material | |
| `Redirect.uasset` | ObjectRedirector | |
| `L_TestMap.umap` | World | |

**Invalid fixtures** — synthetic files created manually:

| File | Content | Expected error |
|---|---|---|
| `bad_magic.bin` | 4 bytes: `00 00 00 00` | `ScanError::InvalidMagic` |
| `truncated.bin` | 4 bytes: `C1 83 2A 9E` (valid magic, then EOF) | `ScanError::UnexpectedEof` |

`bad_magic.bin` and `truncated.bin` can be created with any hex editor or a small Rust script.

**`tests/fixtures/README.md`** must document the UE version used and how to regenerate the files.

> **Note**: The `.gitattributes` binary attribute for these paths should already be set from Issue #1. Verify it is in place before committing the binary files.

## Requirements

- [ ] Copy 6 `.uasset`/`.umap` files from a UE5.1+ project into `tests/fixtures/valid/`
- [ ] Create `tests/fixtures/invalid/bad_magic.bin` (4 bytes: `00 00 00 00`)
- [ ] Create `tests/fixtures/invalid/truncated.bin` (4 bytes: `C1 83 2A 9E`)
- [ ] Update `tests/fixtures/README.md` with the UE version, project name/source, and the expected AssetType for each fixture
- [ ] Verify `.gitattributes` binary attributes are applied (`git check-attr binary tests/fixtures/valid/T_Rock.uasset` should return `set`)

## Related

- Depends on: #1 (fixture directories and `.gitattributes` created)
- Needed by: #5 (scanner header parser tests), #8 (scan_files integration tests)
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 3-4
