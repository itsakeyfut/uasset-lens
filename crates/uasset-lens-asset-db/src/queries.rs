use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::types::Value;
use uasset_lens_shared::AssetPath;

use crate::db::AssetDb;
use crate::error::DbError;
use crate::record::{AssetFilter, AssetRecord, BlueprintRow, ScanSnapshot};

fn parse_asset_record(
    id: i64,
    asset_path: String,
    file_path: String,
    asset_type_json: String,
    file_size: i64,
    last_modified: i64,
) -> Result<AssetRecord, DbError> {
    Ok(AssetRecord {
        id,
        asset_path: AssetPath::new(&asset_path)?,
        file_path: PathBuf::from(file_path),
        asset_type: serde_json::from_str(&asset_type_json)?,
        file_size: file_size as u64,
        last_modified: last_modified as u64,
    })
}

impl AssetDb {
    /// The on-disk schema version, stored in SQLite `PRAGMA user_version`.
    pub fn schema_version(&self) -> Result<i64, DbError> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// The scanner (binary) version that last wrote this DB, if it was recorded.
    pub fn scanner_version(&self) -> Result<Option<String>, DbError> {
        match self.conn.query_row(
            "SELECT value FROM meta WHERE key = 'scanner_version'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Number of indexed assets.
    pub fn asset_count(&self) -> Result<u64, DbError> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))?;
        Ok(n as u64)
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

    pub fn all_assets(&self) -> Result<Vec<AssetRecord>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, asset_path, file_path, asset_type, file_size, last_modified FROM assets",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(id, ap, fp, at, fs, lm)| parse_asset_record(id, ap, fp, at, fs, lm))
        .collect()
    }

    pub fn all_edges(&self) -> Result<Vec<(AssetPath, AssetPath)>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.asset_path, d.to_path \
             FROM dependencies d \
             JOIN assets a ON d.from_id = a.id",
        )?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .map(|r| {
            let (from, to) = r?;
            Ok((AssetPath::new(&from)?, AssetPath::new(&to)?))
        })
        .collect()
    }

    pub fn get_asset(&self, asset_path: &AssetPath) -> Result<Option<AssetRecord>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, asset_path, file_path, asset_type, file_size, last_modified \
             FROM assets WHERE asset_path = ?1",
        )?;
        match stmt.query_row([asset_path.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        }) {
            Ok((id, ap, fp, at, fs, lm)) => parse_asset_record(id, ap, fp, at, fs, lm).map(Some),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    pub fn find_assets(&self, filter: &AssetFilter) -> Result<Vec<AssetRecord>, DbError> {
        let mut conditions: Vec<&str> = Vec::new();
        let mut values: Vec<Value> = Vec::new();

        if let Some(ref at) = filter.asset_type {
            conditions.push("asset_type = ?");
            values.push(Value::Text(serde_json::to_string(at)?));
        }
        if let Some(min_size) = filter.min_size {
            conditions.push("file_size >= ?");
            values.push(Value::Integer(min_size as i64));
        }
        if let Some(max_size) = filter.max_size {
            conditions.push("file_size <= ?");
            values.push(Value::Integer(max_size as i64));
        }

        let sql = format!(
            "SELECT id, asset_path, file_path, asset_type, file_size, last_modified FROM assets{}",
            if conditions.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", conditions.join(" AND "))
            }
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let mut records = stmt
            .query_map(rusqlite::params_from_iter(values), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(id, ap, fp, at, fs, lm)| parse_asset_record(id, ap, fp, at, fs, lm))
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(ref pattern) = filter.path_pattern {
            let matcher = globset::Glob::new(pattern)?.compile_matcher();
            records.retain(|r| matcher.is_match(&r.file_path));
        }

        Ok(records)
    }

    pub fn recent_snapshots(&self, n: usize) -> Result<Vec<ScanSnapshot>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scanned_at, asset_count, total_size, \
                    blueprint_count, avg_node_count, texture_count, texture_size \
             FROM scan_history \
             ORDER BY scanned_at DESC \
             LIMIT ?1",
        )?;
        stmt.query_map([n as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(
            |(
                id,
                scanned_at,
                asset_count,
                total_size,
                blueprint_count,
                avg_node_count,
                texture_count,
                texture_size,
            )| {
                Ok(ScanSnapshot {
                    id,
                    scanned_at: scanned_at as u64,
                    asset_count: asset_count as u64,
                    total_size: total_size as u64,
                    blueprint_count: blueprint_count as u64,
                    avg_node_count,
                    texture_count: texture_count as u64,
                    texture_size: texture_size as u64,
                })
            },
        )
        .collect()
    }

    pub fn load_baseline(&self, name: &str) -> Result<ScanSnapshot, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT sh.id, sh.scanned_at, sh.asset_count, sh.total_size, \
                    sh.blueprint_count, sh.avg_node_count, sh.texture_count, sh.texture_size \
             FROM baselines b \
             JOIN scan_history sh ON b.snapshot_id = sh.id \
             WHERE b.name = ?1",
        )?;
        match stmt.query_row([name], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
            ))
        }) {
            Ok((
                id,
                scanned_at,
                asset_count,
                total_size,
                blueprint_count,
                avg_node_count,
                texture_count,
                texture_size,
            )) => Ok(ScanSnapshot {
                id,
                scanned_at: scanned_at as u64,
                asset_count: asset_count as u64,
                total_size: total_size as u64,
                blueprint_count: blueprint_count as u64,
                avg_node_count,
                texture_count: texture_count as u64,
                texture_size: texture_size as u64,
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(DbError::BaselineNotFound(name.to_owned()))
            }
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    pub fn all_blueprint_metrics(&self) -> Result<Vec<BlueprintRow>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.asset_path, a.asset_type, \
                    bm.node_count, bm.event_tick, bm.cast_count, bm.dep_depth \
             FROM blueprint_metrics bm \
             JOIN assets a ON bm.asset_id = a.id",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(ap, at, nc, et, cc, dd)| {
            Ok(BlueprintRow {
                asset_path: AssetPath::new(&ap)?,
                asset_type: serde_json::from_str(&at)?,
                node_count: nc as u32,
                event_tick_count: et as u32,
                cast_count: cc as u32,
                dependency_depth: dd as u32,
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use uasset_lens_shared::{AssetPath, AssetType};

    fn make_meta(
        asset_path: &str,
        file_path: &str,
        mtime: u64,
    ) -> uasset_lens_scanner::AssetMetadata {
        uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from(file_path),
            last_modified: mtime,
            ..uasset_lens_scanner::make_meta(asset_path, AssetType::Blueprint)
        }
    }

    fn make_meta_full(
        asset_path: &str,
        file_path: &str,
        file_size: u64,
        asset_type: AssetType,
    ) -> uasset_lens_scanner::AssetMetadata {
        uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from(file_path),
            file_size,
            ..uasset_lens_scanner::make_meta(asset_path, asset_type)
        }
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
    fn all_assets_should_return_all_upserted_records() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta("/Game/A", "/proj/Content/A.uasset", 100))
            .unwrap();
        db.upsert_asset(&make_meta("/Game/B", "/proj/Content/B.uasset", 200))
            .unwrap();

        let records = db.all_assets().unwrap();
        assert_eq!(records.len(), 2);
        let paths: Vec<_> = records.iter().map(|r| r.asset_path.as_str()).collect();
        assert!(paths.contains(&"/Game/A"));
        assert!(paths.contains(&"/Game/B"));
    }

    #[test]
    fn all_edges_should_return_correct_from_to_pairs() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let id = db
            .upsert_asset(&make_meta("/Game/A", "/proj/Content/A.uasset", 100))
            .unwrap();
        db.replace_dependencies(id, &[AssetPath::new("/Game/B").unwrap()])
            .unwrap();

        let edges = db.all_edges().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0.as_str(), "/Game/A");
        assert_eq!(edges[0].1.as_str(), "/Game/B");
    }

    #[test]
    fn get_asset_should_return_record_for_existing_path() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta("/Game/A", "/proj/Content/A.uasset", 100))
            .unwrap();

        let record = db
            .get_asset(&AssetPath::new("/Game/A").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(record.asset_path.as_str(), "/Game/A");
        assert_eq!(record.asset_type, AssetType::Blueprint);
    }

    #[test]
    fn get_asset_should_return_none_for_unknown_path() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let result = db
            .get_asset(&AssetPath::new("/Game/NoSuchAsset").unwrap())
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_assets_should_filter_by_asset_type() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/BP_A",
            "/proj/Content/BP_A.uasset",
            1024,
            AssetType::Blueprint,
        ))
        .unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/T_Rock",
            "/proj/Content/T_Rock.uasset",
            1024,
            AssetType::Texture2D,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: Some(AssetType::Blueprint),
            min_size: None,
            max_size: None,
            path_pattern: None,
        };
        let results = db.find_assets(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset_path.as_str(), "/Game/BP_A");
    }

    #[test]
    fn find_assets_should_filter_by_min_size() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/Small",
            "/proj/Content/Small.uasset",
            512,
            AssetType::Blueprint,
        ))
        .unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/Large",
            "/proj/Content/Large.uasset",
            4096,
            AssetType::Blueprint,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: None,
            min_size: Some(1024),
            max_size: None,
            path_pattern: None,
        };
        let results = db.find_assets(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset_path.as_str(), "/Game/Large");
    }

    #[test]
    fn find_assets_should_filter_by_max_size() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/Small",
            "/proj/Content/Small.uasset",
            512,
            AssetType::Blueprint,
        ))
        .unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/Large",
            "/proj/Content/Large.uasset",
            4096,
            AssetType::Blueprint,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: None,
            min_size: None,
            max_size: Some(1024),
            path_pattern: None,
        };
        let results = db.find_assets(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset_path.as_str(), "/Game/Small");
    }

    #[test]
    fn find_assets_should_filter_by_path_pattern() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta(
            "/Game/Characters/BP_Player",
            "/proj/Content/Characters/BP_Player.uasset",
            100,
        ))
        .unwrap();
        db.upsert_asset(&make_meta(
            "/Game/UI/WBP_HUD",
            "/proj/Content/UI/WBP_HUD.uasset",
            100,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: None,
            min_size: None,
            max_size: None,
            path_pattern: Some("**/Characters/**".to_string()),
        };
        let results = db.find_assets(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset_path.as_str(), "/Game/Characters/BP_Player");
    }

    #[test]
    fn find_assets_should_return_empty_when_path_pattern_matches_nothing() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta(
            "/Game/Characters/BP_Player",
            "/proj/Content/Characters/BP_Player.uasset",
            100,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: None,
            min_size: None,
            max_size: None,
            path_pattern: Some("**/Levels/**".to_string()),
        };
        let results = db.find_assets(&filter).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn find_assets_should_apply_combined_filters() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/BP_Big",
            "/proj/Content/BP_Big.uasset",
            4096,
            AssetType::Blueprint,
        ))
        .unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/BP_Small",
            "/proj/Content/BP_Small.uasset",
            512,
            AssetType::Blueprint,
        ))
        .unwrap();
        db.upsert_asset(&make_meta_full(
            "/Game/T_Big",
            "/proj/Content/T_Big.uasset",
            4096,
            AssetType::Texture2D,
        ))
        .unwrap();

        let filter = AssetFilter {
            asset_type: Some(AssetType::Blueprint),
            min_size: Some(1024),
            max_size: None,
            path_pattern: None,
        };
        let results = db.find_assets(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset_path.as_str(), "/Game/BP_Big");
    }

    #[test]
    fn find_assets_should_return_empty_for_no_match() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta("/Game/BP_A", "/proj/Content/BP_A.uasset", 100))
            .unwrap();

        let filter = AssetFilter {
            asset_type: Some(AssetType::StaticMesh),
            min_size: None,
            max_size: None,
            path_pattern: None,
        };
        let results = db.find_assets(&filter).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn load_baseline_should_return_snapshot_for_known_name() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let snapshot_id = db.record_scan_snapshot().unwrap();
        db.save_baseline("main", snapshot_id).unwrap();
        let snap = db.load_baseline("main").unwrap();
        assert_eq!(snap.id, snapshot_id);
    }

    #[test]
    fn load_baseline_should_return_baseline_not_found_for_unknown_name() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let err = db.load_baseline("nonexistent").unwrap_err();
        assert!(
            matches!(err, DbError::BaselineNotFound(_)),
            "expected BaselineNotFound, got {err:?}"
        );
    }

    #[test]
    fn recent_snapshots_should_return_empty_when_no_history() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        let snaps = db.recent_snapshots(10).unwrap();
        assert!(snaps.is_empty());
    }

    #[test]
    fn recent_snapshots_should_return_at_most_n_rows() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.record_scan_snapshot().unwrap();
        db.record_scan_snapshot().unwrap();
        db.record_scan_snapshot().unwrap();
        let snaps = db.recent_snapshots(2).unwrap();
        assert_eq!(snaps.len(), 2);
    }

    #[test]
    fn recent_snapshots_should_return_most_recent_first() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.conn
            .execute(
                "INSERT INTO scan_history \
                 (scanned_at, asset_count, total_size, blueprint_count, avg_node_count, texture_count, texture_size) \
                 VALUES (100, 0, 0, 0, 0.0, 0, 0)",
                [],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO scan_history \
                 (scanned_at, asset_count, total_size, blueprint_count, avg_node_count, texture_count, texture_size) \
                 VALUES (200, 0, 0, 0, 0.0, 0, 0)",
                [],
            )
            .unwrap();
        let snaps = db.recent_snapshots(2).unwrap();
        assert_eq!(snaps.len(), 2);
        assert!(snaps[0].scanned_at >= snaps[1].scanned_at);
    }

    #[test]
    fn all_blueprint_metrics_should_return_empty_when_no_blueprint_metrics_stored() {
        let db = AssetDb::open(Path::new(":memory:")).unwrap();
        db.upsert_asset(&make_meta(
            "/Game/T_Rock",
            "/proj/Content/T_Rock.uasset",
            100,
        ))
        .unwrap();

        let rows = db.all_blueprint_metrics().unwrap();
        assert!(
            rows.is_empty(),
            "assets with no blueprint_metrics should not appear"
        );
    }
}
