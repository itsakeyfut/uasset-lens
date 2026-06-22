use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use uasset_lens_asset_db::AssetRecord;

/// Error returned when `--format sarif` is used on a command that does not produce violations.
pub(crate) fn sarif_not_supported() -> anyhow::Error {
    anyhow::anyhow!("--format sarif is only supported by the check, lint, and budget commands")
}

/// Serializes `value` as pretty JSON and prints it as a single stdout value (the `--format json`
/// stdout-purity contract). `ctx` labels the serialization error.
pub(crate) fn emit_json<T: Serialize>(value: &T, ctx: &'static str) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value).context(ctx)?);
    Ok(())
}

/// Maps each asset's `/Game/...` path to its on-disk file path, for annotation / SARIF uri
/// resolution. Borrows from `assets`, so the map lives as long as the slice.
pub(crate) fn path_lookup(assets: &[AssetRecord]) -> HashMap<&str, &Path> {
    assets
        .iter()
        .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
        .collect()
}
