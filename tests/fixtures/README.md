# Test Fixtures

This directory contains binary `.uasset` / `.umap` / `.bin` files used in scanner integration tests.

## Directory Structure

```
fixtures/
  valid/          # Well-formed .uasset / .umap files for happy-path tests
  invalid/        # Synthetically crafted files for error-path tests
```

## `valid/` — Fixtures

See [`valid/README.md`](valid/README.md) for the full catalogue, source filenames, and regeneration steps.

| File | Asset Type | Status |
|------|-----------|--------|
| `BP_Simple.uasset` | Blueprint | ✅ |
| `T_Rock.uasset` | Texture2D | ✅ |
| `SM_Cube.uasset` | StaticMesh | ✅ |
| `M_Basic.uasset` | Material | ✅ |
| `L_TestMap.umap` | World | ✅ |
| `Redirect.uasset` | ObjectRedirector | ✅ |

## `invalid/` — Error-case fixtures

Synthetically crafted files — do not require the UE editor.

| File | Content | Expected Error |
|------|---------|---------------|
| `bad_magic.bin` | 4 bytes: `00 00 00 00` | `ScanError::InvalidMagic` |
| `truncated.bin` | 4 bytes: `C1 83 2A 9E` (valid magic, then EOF) | `ScanError::UnexpectedEof` |
