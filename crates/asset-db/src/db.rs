use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use shared::AssetPath;

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

    /// Stores or replaces the asset record for `meta` and returns the row id.
    /// Uses INSERT OR REPLACE, so existing dependencies are cascade-deleted on replace;
    /// call `replace_dependencies` with the returned id to re-insert them.
    pub fn upsert_asset(&self, meta: &scanner::AssetMetadata) -> Result<i64, DbError> {
        let asset_type = serde_json::to_string(&meta.asset_type)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO assets \
             (asset_path, file_path, asset_type, file_size, last_modified) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                meta.asset_path.as_str(),
                meta.file_path.to_string_lossy().as_ref(),
                asset_type,
                meta.file_size as i64,
                meta.last_modified as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_asset(&self, asset_path: &AssetPath) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM assets WHERE asset_path = ?1",
            [asset_path.as_str()],
        )?;
        Ok(())
    }

    // Non-atomic: caller must wrap with upsert_asset in a transaction to avoid partial dep state.
    pub fn replace_dependencies(
        &self,
        from_id: i64,
        to_paths: &[AssetPath],
    ) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM dependencies WHERE from_id = ?1", [from_id])?;
        let mut stmt = self
            .conn
            .prepare("INSERT INTO dependencies (from_id, to_path) VALUES (?1, ?2)")?;
        for path in to_paths {
            stmt.execute(rusqlite::params![from_id, path.as_str()])?;
        }
        Ok(())
    }

    /// Returns the subset of `files` that are new (not in DB) or whose mtime differs.
    pub fn filter_changed(&self, files: &[(PathBuf, u64)]) -> Result<Vec<PathBuf>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_path, last_modified FROM assets")?;
        let known: HashMap<String, u64> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?
            .collect::<Result<_, _>>()?;

        Ok(files
            .iter()
            .filter(|(path, mtime)| {
                let key = path.to_string_lossy();
                known
                    .get(key.as_ref())
                    .is_none_or(|&stored| stored != *mtime)
            })
            .map(|(path, _)| {
                // clone required: result owns the path; files yields references
                path.clone()
            })
            .collect())
    }

    pub fn all_known_files(&self) -> Result<Vec<PathBuf>, DbError> {
        let mut stmt = self.conn.prepare("SELECT file_path FROM assets")?;
        let paths = stmt
            .query_map([], |row| {
                let s: String = row.get(0)?;
                Ok(PathBuf::from(s))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paths)
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
    use shared::{AssetPath, AssetType};

    fn make_meta(asset_path: &str, file_path: &str, mtime: u64) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path: PathBuf::from(file_path),
            asset_type: AssetType::Blueprint,
            file_size: 1024,
            last_modified: mtime,
            dependencies: vec![],
        }
    }

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
    fn filter_changed_should_return_new_file_not_in_db() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let files = vec![(PathBuf::from("/proj/Content/BP_Test.uasset"), 100u64)];
        let result = db.filter_changed(&files).unwrap();
        assert_eq!(result, vec![PathBuf::from("/proj/Content/BP_Test.uasset")]);
    }

    #[test]
    fn filter_changed_should_return_file_with_changed_mtime() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta(
            "/Game/BP_Test",
            "/proj/Content/BP_Test.uasset",
            100,
        ))
        .unwrap();

        let files = vec![(PathBuf::from("/proj/Content/BP_Test.uasset"), 200u64)];
        let result = db.filter_changed(&files).unwrap();
        assert_eq!(result, vec![PathBuf::from("/proj/Content/BP_Test.uasset")]);
    }

    #[test]
    fn filter_changed_should_not_return_file_with_unchanged_mtime() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta(
            "/Game/BP_Test",
            "/proj/Content/BP_Test.uasset",
            100,
        ))
        .unwrap();

        let files = vec![(PathBuf::from("/proj/Content/BP_Test.uasset"), 100u64)];
        let result = db.filter_changed(&files).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn upsert_then_delete_should_cascade_remove_dependencies() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let meta = make_meta("/Game/BP_Test", "/proj/Content/BP_Test.uasset", 100);
        let id = db.upsert_asset(&meta).unwrap();

        let deps = vec![AssetPath::new("/Game/Dep").unwrap()];
        db.replace_dependencies(id, &deps).unwrap();

        let dep_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM dependencies WHERE from_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dep_count, 1);

        db.delete_asset(&meta.asset_path).unwrap();

        let asset_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM assets WHERE asset_path = ?1",
                [meta.asset_path.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(asset_count, 0);

        let dep_count_after: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM dependencies WHERE from_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dep_count_after, 0);
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
