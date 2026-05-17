use rusqlite::Connection;
use shared::AssetPath;

use crate::db::AssetDb;
use crate::error::DbError;

fn upsert_asset_conn(conn: &Connection, meta: &scanner::AssetMetadata) -> Result<i64, DbError> {
    let asset_type = serde_json::to_string(&meta.asset_type)?;
    conn.execute(
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
    let id = conn.last_insert_rowid();

    // INSERT OR REPLACE on assets cascade-deletes the old blueprint_metrics row;
    // re-insert if the scan produced metrics for this asset.
    if let Some(ref bm) = meta.blueprint_metrics {
        conn.execute(
            "INSERT INTO blueprint_metrics \
             (asset_id, node_count, event_tick, cast_count, dep_depth) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id,
                bm.node_count as i64,
                bm.event_tick_count as i64,
                bm.cast_count as i64,
                bm.dependency_depth as i64,
            ],
        )?;
    }

    Ok(id)
}

impl AssetDb {
    /// Stores or replaces the asset record for `meta` and returns the row id.
    /// Uses INSERT OR REPLACE, so existing dependencies are cascade-deleted on replace;
    /// call `replace_dependencies` with the returned id to re-insert them.
    pub fn upsert_asset(&self, meta: &scanner::AssetMetadata) -> Result<i64, DbError> {
        upsert_asset_conn(&self.conn, meta)
    }

    pub fn upsert_all(&mut self, assets: &[scanner::AssetMetadata]) -> Result<(), DbError> {
        let tx = self.conn.transaction()?;
        for meta in assets {
            let id = upsert_asset_conn(&tx, meta)?;
            tx.execute("DELETE FROM dependencies WHERE from_id = ?1", [id])?;
            for dep in &meta.dependencies {
                tx.execute(
                    "INSERT OR IGNORE INTO dependencies (from_id, to_path, is_soft) VALUES (?1, ?2, 0)",
                    rusqlite::params![id, dep.as_str()],
                )?;
            }
            // OR IGNORE: if the same path is already a hard dep (is_soft=0), the soft insert
            // is skipped. The hard reference is the stronger claim so is_soft=0 is correct.
            for dep in &meta.soft_dependencies {
                tx.execute(
                    "INSERT OR IGNORE INTO dependencies (from_id, to_path, is_soft) VALUES (?1, ?2, 1)",
                    rusqlite::params![id, dep.as_str()],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
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
        // OR IGNORE: a single .uasset can reference the same package via multiple
        // import entries; duplicate edges are silently skipped.
        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO dependencies (from_id, to_path, is_soft) VALUES (?1, ?2, 0)",
        )?;
        for path in to_paths {
            stmt.execute(rusqlite::params![from_id, path.as_str()])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};
    use std::path::{Path, PathBuf};

    #[test]
    fn upsert_then_delete_should_cascade_remove_dependencies() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/BP_Test.uasset"),
            last_modified: 100,
            ..scanner::make_meta("/Game/BP_Test", AssetType::Blueprint)
        };
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
    fn upsert_all_should_insert_multiple_assets_and_their_dependencies() {
        let mut db = AssetDb::open(Path::new(":memory:")).unwrap();
        let assets = vec![
            scanner::AssetMetadata {
                file_path: PathBuf::from("/proj/Content/A.uasset"),
                file_size: 1024,
                last_modified: 100,
                dependencies: vec![AssetPath::new("/Game/Dep").unwrap()],
                ..scanner::make_meta("/Game/A", AssetType::Blueprint)
            },
            scanner::AssetMetadata {
                file_path: PathBuf::from("/proj/Content/B.uasset"),
                file_size: 2048,
                last_modified: 200,
                ..scanner::make_meta("/Game/B", AssetType::Texture2D)
            },
        ];
        db.upsert_all(&assets).unwrap();

        let records = db.all_assets().unwrap();
        assert_eq!(records.len(), 2);

        let edges = db.all_edges().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0.as_str(), "/Game/A");
        assert_eq!(edges[0].1.as_str(), "/Game/Dep");
    }

    #[test]
    fn upsert_asset_should_store_blueprint_metrics_when_present() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let bm = scanner::BlueprintMetrics {
            node_count: 42,
            event_tick_count: 1,
            cast_count: 3,
            dependency_depth: 2,
        };
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/BP_Test.uasset"),
            file_size: 1024,
            last_modified: 100,
            blueprint_metrics: Some(bm),
            ..scanner::make_meta("/Game/BP_Test", AssetType::Blueprint)
        };
        db.upsert_asset(&meta).unwrap();

        let rows = db.all_blueprint_metrics().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].asset_path.as_str(), "/Game/BP_Test");
        assert_eq!(rows[0].node_count, 42);
        assert_eq!(rows[0].event_tick_count, 1);
        assert_eq!(rows[0].cast_count, 3);
        assert_eq!(rows[0].dependency_depth, 2);
    }

    #[test]
    fn upsert_asset_should_not_store_blueprint_metrics_when_absent() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/BP_NoMetrics.uasset"),
            last_modified: 100,
            ..scanner::make_meta("/Game/BP_NoMetrics", AssetType::Blueprint)
        };
        db.upsert_asset(&meta).unwrap();

        let rows = db.all_blueprint_metrics().unwrap();
        assert!(rows.is_empty());
    }
}
