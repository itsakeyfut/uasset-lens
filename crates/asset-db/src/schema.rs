use crate::db::AssetDb;
use crate::error::DbError;

impl AssetDb {
    pub(crate) fn init_schema(&self) -> Result<(), DbError> {
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

             CREATE TABLE IF NOT EXISTS blueprint_metrics (
                 asset_id   INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
                 node_count INTEGER NOT NULL,
                 event_tick INTEGER NOT NULL,
                 cast_count INTEGER NOT NULL,
                 dep_depth  INTEGER NOT NULL
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
    use std::path::Path;

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
    fn open_should_create_schema_with_blueprint_metrics_table() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let mut stmt = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            names.contains(&"blueprint_metrics".to_string()),
            "blueprint_metrics table should be created by init_schema"
        );
    }
}
