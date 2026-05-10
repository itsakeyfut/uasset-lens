use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid asset path stored in database: {0}")]
    InvalidAssetPath(#[from] shared::AssetPathError),
    #[error("invalid glob pattern: {0}")]
    Glob(#[from] globset::Error),
    #[error("database not found: {0}")]
    NotFound(PathBuf),
}
