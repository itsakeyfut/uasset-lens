use std::path::Path;

use rusqlite::Connection;

use crate::DbError;

/// On-disk schema version, stored in SQLite `PRAGMA user_version`. Bump this when the schema
/// changes in a way that requires a re-scan or migration; `doctor` reports a mismatch.
pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub struct AssetDb {
    pub(crate) conn: Connection,
}

impl AssetDb {
    /// Opens or creates the database at `db_path`. Used by the scan command which must
    /// create the DB on first run.
    pub fn open(db_path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(db_path)?;
        let db = AssetDb { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Opens an existing database. Returns `DbError::NotFound` if the file does not exist,
    /// preventing silent creation of an empty DB on commands that require prior scan data.
    pub fn open_existing(db_path: &Path) -> Result<Self, DbError> {
        if !db_path.exists() {
            return Err(DbError::NotFound(db_path.to_path_buf()));
        }
        Self::open(db_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_existing_should_return_not_found_when_file_does_not_exist() {
        let path =
            std::env::temp_dir().join(format!("uasset_lens_db_missing_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let result = AssetDb::open_existing(&path);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn open_existing_should_succeed_when_file_exists() {
        let path =
            std::env::temp_dir().join(format!("uasset_lens_db_exists_{}.db", std::process::id()));
        AssetDb::open(&path).unwrap();
        let result = AssetDb::open_existing(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn open_should_succeed_on_second_open_of_existing_db() {
        let dir = std::env::temp_dir();
        let db_path = dir.join(format!("uasset_lens_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);

        AssetDb::open(&db_path).unwrap();
        AssetDb::open(&db_path).unwrap();
        let _ = std::fs::remove_file(&db_path);
    }
}
