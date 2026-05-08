# Test Fixtures

This directory contains binary `.uasset` / `.umap` / `.bin` files used in scanner integration tests.

## UE Version

<!-- TODO: Record the Unreal Engine version used to generate each fixture -->
<!-- Example: UE 5.3.2 -->

## Directory Structure

```
fixtures/
  valid/          # Well-formed .uasset / .umap files for happy-path tests
  invalid/        # Synthetically crafted files for error-path tests
```

## `valid/` — Expected fixtures (to be added in Issue #3)

| File | Asset Type | Notes |
|------|-----------|-------|
| `BP_Simple.uasset` | Blueprint | Has Import references |
| `T_Rock.uasset` | Texture2D | |
| `SM_Cube.uasset` | StaticMesh | |
| `M_Basic.uasset` | Material | |
| `Redirect.uasset` | ObjectRedirector | |
| `L_TestMap.umap` | World | |

## `invalid/` — Error-case fixtures (to be added in Issue #3)

| File | Expected Error |
|------|---------------|
| `bad_magic.bin` | `ScanError::InvalidMagic` |
| `truncated.bin` | `ScanError::UnexpectedEof` |

## How to Generate `valid/` Fixtures

1. Open the Unreal Engine editor (UE 5.x).
2. Create minimal assets of each required type in a blank project.
3. Save and close the editor.
4. Copy the `.uasset` / `.umap` files from `<Project>/Content/` into `tests/fixtures/valid/`.
5. Record the UE version above.

## How to Generate `invalid/` Fixtures

These are synthetically crafted and do not require the UE editor.
See `crates/scanner/tests/` for the generation scripts or inline byte arrays.
