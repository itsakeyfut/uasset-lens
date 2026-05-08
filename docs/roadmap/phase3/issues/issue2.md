# `.uasset-lens.toml` — project configuration file support

## Summary

Implement configuration file loading for `.uasset-lens.toml` and apply `exclude_paths`
to the `scan` command's directory enumeration.
Complete when assets under an excluded path are absent from the scan results.

## Design Notes

**File location:** `<project_dir>/.uasset-lens.toml`
Auto-discovered at the start of every command. Missing file is not an error — fall back to
default (empty) config silently.

**Initial schema (`[scan]` section only):**

```toml
[scan]
exclude_paths = ["Content/Dev/", "Content/Test/"]
```

`exclude_paths` entries are prefix strings relative to `content_root`.
During walkdir, skip any directory whose path starts with one of these prefixes.

**`ConfigFile` struct** (defined in `cli` crate or a `config` submodule):

```rust
#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    pub scan: ScanConfig,
}

#[derive(Default, serde::Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}
```

**Config loading:**

```rust
fn load_config(project_dir: &Path) -> ConfigFile {
    let path = project_dir.join(".uasset-lens.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}
```

Use `unwrap_or_default()` — any parse error silently falls back to defaults.

## Requirements

- [ ] Define `ConfigFile` and `ScanConfig` structs with `serde::Deserialize` and `Default`
- [ ] Implement `load_config(project_dir: &Path) -> ConfigFile`
- [ ] Apply `exclude_paths` in the `scan` command: skip walkdir entries whose path starts with any excluded prefix (compared against content_root-relative path)
- [ ] Unit test: valid `.uasset-lens.toml` is parsed correctly
- [ ] Unit test: missing file returns `ConfigFile::default()`
- [ ] Unit test: malformed TOML returns `ConfigFile::default()` (no panic)
- [ ] Integration test: file under `Content/Dev/` is not indexed after scan with exclude config

## Related

- Next: Issue #3 (redirectors command uses config)
- Docs: `docs/roadmap/phase3/ROADMAP.md` — Task 2
