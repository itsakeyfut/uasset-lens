# Test Strategy

## Philosophy

`.uasset` files are binary assets that are normally committed to VCS (Git / SVN) in UE development.
uasset-lens test fixtures follow the same convention — real `.uasset` files committed to the repository.
This lets the parser be verified on CI (multi-OS) without requiring a UE installation.

---

## Fixture Layout

```
tests/
  fixtures/
    valid/
      BP_Simple.uasset          # Blueprint (with imports and exports)
      T_Rock_D.uasset            # Texture2D
      SM_Cube.uasset             # StaticMesh
      M_Basic.uasset             # Material
      OldName.uasset             # ObjectRedirector
      L_TestMap.umap             # World
    invalid/
      bad_magic.bin              # first 4 bytes invalid (0x00000000)
      truncated.bin              # truncated mid-header
```

- `valid/`: minimal `.uasset` / `.umap` files exported from a real UE5 project
- `invalid/`: synthetic binaries for error cases (may also be defined inline in test code)

---

## Test Classification

### Unit tests (`#[cfg(test)]`)

Written inside each parser module. Target individual parsing functions.

```rust
// crates/scanner/src/parser/header.rs
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_invalid_magic() {
        let data = &[0x00u8; 32];
        assert!(matches!(parse_header(data), Err(ScanError::InvalidMagic(_))));
    }
}
```

### Integration tests (`tests/` directory)

Placed in the crate root's `tests/` directory. Test the public API against real fixtures.

```rust
// crates/scanner/tests/integration.rs
#[test]
fn parses_blueprint_fixture() {
    let root    = Path::new("tests/fixtures/valid");
    let results = scanner::scan_files(&[root.join("BP_Simple.uasset")], root);
    let meta    = &results.assets[0];
    assert_eq!(meta.asset_type, AssetType::Blueprint);
    assert!(!meta.dependencies.is_empty());
}
```

### Error cases

Place error-case fixtures in `tests/fixtures/invalid/`,
or define them inline as byte arrays (preferred for short inputs).

```rust
#[test]
fn rejects_truncated_file() {
    let data = b"\xC1\x83\x2A\x9E"; // magic only, rest missing
    let path = write_temp_file(data);
    let result = scanner::scan_files(&[path], Path::new("/"));
    assert_eq!(result.skipped.len(), 1);
    assert!(matches!(result.skipped[0].reason, ScanError::UnexpectedEof));
}
```

---

## CI Configuration

- Multi-OS testing: Windows / macOS / Linux (GitHub Actions matrix)
- Fixtures are checked out with a normal `git checkout` — no additional setup required
- `.uasset` files must have the `binary` attribute in `.gitattributes` to prevent LF conversion

```
# .gitattributes
tests/fixtures/**/*.uasset binary
tests/fixtures/**/*.umap   binary
```

---

## Fixture Maintenance

- If a UE update changes the `.uasset` format, regenerate the fixtures and commit them in a PR
- Document the UE version and generation steps in `tests/fixtures/README.md`
