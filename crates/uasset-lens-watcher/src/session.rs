use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use uasset_lens_shared::AssetPath;

use crate::{WatchEvent, WatchEventKind, Watcher};

#[derive(Debug, thiserror::Error)]
pub enum WatchSessionError {
    #[error("database error: {0}")]
    Db(#[from] uasset_lens_asset_db::DbError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub struct WatchSession {
    db: uasset_lens_asset_db::AssetDb,
    content_root: PathBuf,
    external_roots: Vec<String>,
    last_dead: HashSet<AssetPath>,
    last_cycles: Vec<Vec<AssetPath>>,
}

impl WatchSession {
    pub fn new(db: uasset_lens_asset_db::AssetDb, content_root: PathBuf, external_roots: Vec<String>) -> Self {
        Self {
            db,
            content_root,
            external_roots,
            last_dead: HashSet::new(),
            last_cycles: Vec::new(),
        }
    }

    /// Snapshot current DB state so the first batch is diffed against existing problems.
    pub fn init(&mut self) -> Result<(), WatchSessionError> {
        let graph = build_graph(&self.db, &self.external_roots)?;
        self.last_dead = uasset_lens_dead_asset_detector::detect(&graph, &[])
            .into_iter()
            .collect();
        self.last_cycles = graph.find_cycles();
        Ok(())
    }

    /// Loop until `stop` is set, processing each debounced batch as it arrives.
    pub fn run(
        &mut self,
        watcher: &Watcher,
        out: &mut dyn io::Write,
        stop: &Arc<AtomicBool>,
    ) -> Result<(), WatchSessionError> {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if let Some(batch) = watcher.next_batch_timeout(Duration::from_millis(200)) {
                self.process_batch(&batch, out)?;
            }
        }
        Ok(())
    }

    /// Apply a debounced batch of events: delete removed assets, re-scan changed/created
    /// assets, then emit newly detected dead assets and cycles to `out`.
    /// Exposed as `pub` so unit tests can inject synthetic events without a real Watcher.
    pub fn process_batch(
        &mut self,
        batch: &[WatchEvent],
        out: &mut dyn io::Write,
    ) -> Result<(), WatchSessionError> {
        let mut to_scan: Vec<PathBuf> = Vec::new();
        let mut to_delete: Vec<PathBuf> = Vec::new();

        for event in batch {
            match event.kind {
                WatchEventKind::Changed | WatchEventKind::Created => {
                    to_scan.extend_from_slice(&event.paths);
                }
                WatchEventKind::Deleted => {
                    to_delete.extend_from_slice(&event.paths);
                }
            }
        }

        for path in &to_delete {
            match AssetPath::from_fs_path(&self.content_root, path) {
                Ok(asset_path) => {
                    self.db.delete_asset(&asset_path)?;
                    tracing::debug!(path = %path.display(), "deleted asset from db");
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), reason = %e, "cannot map deleted path to asset path; skipping");
                }
            }
        }

        if !to_scan.is_empty() {
            let result = uasset_lens_scanner::scan_files(&to_scan, &self.content_root);
            for skipped in &result.skipped {
                tracing::warn!(path = %skipped.file_path.display(), reason = %skipped.reason, "skipped during re-scan");
            }
            self.db.upsert_all(&result.assets)?;
            tracing::info!(
                count = result.assets.len(),
                "re-scanned and upserted assets"
            );
        }

        let graph = build_graph(&self.db, &self.external_roots)?;
        let new_dead: HashSet<AssetPath> = uasset_lens_dead_asset_detector::detect(&graph, &[])
            .into_iter()
            .collect();
        let new_cycles = graph.find_cycles();

        for path in new_dead.difference(&self.last_dead) {
            writeln!(out, "[new dead asset] {}", path.as_str())?;
        }

        let old_cycle_keys: HashSet<String> =
            self.last_cycles.iter().map(|c| cycle_key(c)).collect();
        for cycle in &new_cycles {
            if !old_cycle_keys.contains(&cycle_key(cycle)) {
                let parts: Vec<&str> = cycle.iter().map(|p| p.as_str()).collect();
                writeln!(out, "[new cycle] {}", parts.join(" \u{2192} "))?;
            }
        }

        self.last_dead = new_dead;
        self.last_cycles = new_cycles;
        Ok(())
    }
}

fn build_graph(
    db: &uasset_lens_asset_db::AssetDb,
    external_roots: &[String],
) -> Result<uasset_lens_dependency_graph::DependencyGraph, WatchSessionError> {
    let records = db.all_assets()?;
    let nodes = records
        .iter()
        .map(|r| uasset_lens_dependency_graph::AssetNode {
            path: r.asset_path.clone(),       // clone required: AssetPath is not Copy
            asset_type: r.asset_type.clone(), // clone required: AssetType is not Copy
        })
        .collect::<Vec<_>>();
    let edges = db.all_edges()?;
    Ok(uasset_lens_dependency_graph::DependencyGraph::build(
        nodes,
        edges,
        external_roots,
    ))
}

// Canonical string key for a cycle, independent of member ordering.
fn cycle_key(cycle: &[AssetPath]) -> String {
    let mut sorted: Vec<&str> = cycle.iter().map(|p| p.as_str()).collect();
    sorted.sort();
    sorted.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/valid");

    fn make_session() -> WatchSession {
        let db = uasset_lens_asset_db::AssetDb::open(Path::new(":memory:")).unwrap();
        let content_root = PathBuf::from(FIXTURES_DIR);
        WatchSession::new(db, content_root, vec![])
    }

    fn created_event(path: PathBuf) -> WatchEvent {
        WatchEvent {
            kind: WatchEventKind::Created,
            paths: vec![path],
        }
    }

    fn deleted_event(path: PathBuf) -> WatchEvent {
        WatchEvent {
            kind: WatchEventKind::Deleted,
            paths: vec![path],
        }
    }

    #[test]
    fn process_batch_should_print_new_dead_asset_on_created_event() {
        let mut session = make_session();
        let t_rock = PathBuf::from(FIXTURES_DIR).join("T_Rock.uasset");

        let mut out: Vec<u8> = Vec::new();
        session
            .process_batch(&[created_event(t_rock)], &mut out)
            .unwrap();

        let output = String::from_utf8(out).unwrap();
        assert!(
            output.contains("[new dead asset]"),
            "expected '[new dead asset]' in output, got: {output:?}"
        );
    }

    #[test]
    fn process_batch_should_not_reprint_dead_asset_already_tracked() {
        let mut session = make_session();
        let t_rock = PathBuf::from(FIXTURES_DIR).join("T_Rock.uasset");

        // Pre-populate last_dead so T_Rock is already known.
        let asset_path = AssetPath::from_fs_path(&PathBuf::from(FIXTURES_DIR), &t_rock).unwrap();
        session.last_dead.insert(asset_path);

        let mut out: Vec<u8> = Vec::new();
        session
            .process_batch(&[created_event(t_rock)], &mut out)
            .unwrap();

        let output = String::from_utf8(out).unwrap();
        assert!(
            !output.contains("[new dead asset]"),
            "expected no '[new dead asset]' for already-tracked path, got: {output:?}"
        );
    }

    #[test]
    fn process_batch_should_remove_asset_from_db_on_deleted_event() {
        let mut session = make_session();
        let t_rock = PathBuf::from(FIXTURES_DIR).join("T_Rock.uasset");

        // Seed the DB with T_Rock so there is something to delete.
        let scan = uasset_lens_scanner::scan_files(std::slice::from_ref(&t_rock), &PathBuf::from(FIXTURES_DIR));
        session.db.upsert_all(&scan.assets).unwrap();
        assert_eq!(
            session.db.all_assets().unwrap().len(),
            1,
            "DB should have 1 asset before delete"
        );

        let mut out: Vec<u8> = Vec::new();
        session
            .process_batch(&[deleted_event(t_rock)], &mut out)
            .unwrap();

        assert!(
            session.db.all_assets().unwrap().is_empty(),
            "DB should be empty after deleting the asset"
        );
    }
}
