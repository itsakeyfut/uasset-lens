use std::path::Path;

use rusqlite::Connection;

use crate::DbError;

pub struct AssetDb {
    pub(crate) conn: Connection,
}

impl AssetDb {
    pub fn open(db_path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(db_path)?;
        let db = AssetDb { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS assets (
                 id            INTEGER PRIMARY KEY,
                 asset_path    TEXT    UNIQUE NOT NULL,
                 file_path     TEXT    NOT NULL,
                 asset_type    TEXT    NOT NULL,
                 file_size     INTEGER NOT NULL,
                 last_modified INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS dependencies (
                 from_id  INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
                 to_path  TEXT    NOT NULL,
                 PRIMARY KEY (from_id, to_path)
             );

             CREATE INDEX IF NOT EXISTS idx_assets_last_modified ON assets(last_modified);
             CREATE INDEX IF NOT EXISTS idx_assets_asset_type    ON assets(asset_type);
             CREATE INDEX IF NOT EXISTS idx_deps_to_path         ON dependencies(to_path);",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_should_create_schema_in_memory_db() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();

        let mut stmt = db
            .conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type IN ('table', 'index') \
                 ORDER BY name",
            )
            .unwrap();

        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(names.contains(&"assets".to_string()));
        assert!(names.contains(&"dependencies".to_string()));
        assert!(names.contains(&"idx_assets_asset_type".to_string()));
        assert!(names.contains(&"idx_assets_last_modified".to_string()));
        assert!(names.contains(&"idx_deps_to_path".to_string()));
    }

    #[test]
    fn open_should_succeed_on_second_open_of_existing_db() {
        let dir = std::env::temp_dir();
        // process ID makes the filename unique across concurrent CI builds on the same machine
        let db_path = dir.join(format!("uasset_lens_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);

        AssetDb::open(&db_path).unwrap();
        AssetDb::open(&db_path).unwrap();
        let _ = std::fs::remove_file(&db_path);
    }
}
