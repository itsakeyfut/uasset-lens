#![doc = include_str!("../README.md")]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use uasset_lens_shared::{AssetPath, AssetType};

#[derive(Debug, Clone, PartialEq)]
pub struct MetricsDelta {
    pub node_count_delta: i32,
    pub event_tick_count_delta: i32,
}

#[derive(Debug, Clone)]
pub struct AssetDiff {
    pub asset_path: AssetPath,
    pub deps_added: Vec<AssetPath>,
    pub deps_removed: Vec<AssetPath>,
    pub type_changed: Option<(AssetType, AssetType)>,
    pub metrics_delta: Option<MetricsDelta>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitDiffError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Compare the current on-disk version of `asset_path` against the HEAD-committed version.
/// Returns `None` when the file is untracked, the repo has no prior commit, or the scan fails.
pub fn diff_asset(
    asset_path: &AssetPath,
    project_dir: &Path,
    content_root: &Path,
) -> Result<Option<AssetDiff>, GitDiffError> {
    let file_path = match find_fs_path(asset_path, content_root) {
        Some(p) => p,
        None => return Ok(None),
    };
    let git_root = match find_git_root(project_dir) {
        Some(r) => r,
        None => return Ok(None),
    };
    let relative_str = match file_path.strip_prefix(&git_root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"), // Windows path → git-style
        Err(_) => return Ok(None),
    };

    let output = Command::new("git")
        .args(["show", &format!("HEAD:{relative_str}")])
        .current_dir(project_dir)
        .output()?;

    if !output.status.success() {
        tracing::debug!(path = %relative_str, "file not in HEAD; skipping diff");
        return Ok(None);
    }

    let temp_dir = tempfile::TempDir::new()?;
    let Some(filename) = file_path.file_name() else {
        return Ok(None);
    };
    let temp_file = temp_dir.path().join(filename);
    std::fs::write(&temp_file, &output.stdout)?;

    let old_result = uasset_lens_scanner::scan_files(&[temp_file], temp_dir.path());
    let new_result = uasset_lens_scanner::scan_files(&[file_path], content_root);

    let old_meta = match old_result.assets.into_iter().next() {
        Some(m) => m,
        None => return Ok(None),
    };
    let new_meta = match new_result.assets.into_iter().next() {
        Some(m) => m,
        None => return Ok(None),
    };

    Ok(Some(compute_diff(
        asset_path.clone(), // clone required: asset_path is borrowed, compute_diff takes owned AssetPath
        old_meta,
        new_meta,
    )))
}

fn find_fs_path(asset_path: &AssetPath, content_root: &Path) -> Option<PathBuf> {
    let relative = asset_path.as_str().strip_prefix("/Game/")?;
    let mut base = content_root.to_path_buf();
    for component in relative.split('/') {
        base.push(component);
    }
    for ext in ["uasset", "umap"] {
        let p = base.with_extension(ext);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn find_git_root(project_dir: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(project_dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path_str = String::from_utf8(out.stdout).ok()?;
    Some(PathBuf::from(path_str.trim()))
}

pub(crate) fn compute_diff(
    asset_path: AssetPath,
    old: uasset_lens_scanner::AssetMetadata,
    new: uasset_lens_scanner::AssetMetadata,
) -> AssetDiff {
    let old_deps: HashSet<&AssetPath> = old.dependencies.iter().collect();
    let new_deps: HashSet<&AssetPath> = new.dependencies.iter().collect();

    let mut deps_added: Vec<AssetPath> = new_deps
        .difference(&old_deps)
        .map(|p| (*p).clone()) // clone required: converting &&AssetPath reference to owned
        .collect();
    deps_added.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let mut deps_removed: Vec<AssetPath> = old_deps
        .difference(&new_deps)
        .map(|p| (*p).clone()) // clone required: converting &&AssetPath reference to owned
        .collect();
    deps_removed.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let type_changed = if old.asset_type != new.asset_type {
        Some((old.asset_type, new.asset_type))
    } else {
        None
    };

    let metrics_delta = match (old.blueprint_metrics, new.blueprint_metrics) {
        (Some(old_m), Some(new_m)) => Some(MetricsDelta {
            node_count_delta: new_m.node_count as i32 - old_m.node_count as i32,
            event_tick_count_delta: new_m.event_tick_count as i32 - old_m.event_tick_count as i32,
        }),
        _ => None,
    };

    AssetDiff {
        asset_path,
        deps_added,
        deps_removed,
        type_changed,
        metrics_delta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_asset_path(s: &str) -> AssetPath {
        AssetPath::new(s).unwrap()
    }

    fn make_meta(
        asset_path: AssetPath,
        asset_type: AssetType,
        dependencies: Vec<AssetPath>,
        blueprint_metrics: Option<uasset_lens_scanner::BlueprintMetrics>,
    ) -> uasset_lens_scanner::AssetMetadata {
        uasset_lens_scanner::AssetMetadata {
            asset_path,
            file_path: PathBuf::new(),
            asset_type,
            file_size: 0,
            last_modified: 0,
            dependencies,
            soft_dependencies: vec![],
            blueprint_metrics,
            material_texture_samples: None,
            texture_metadata: None,
        }
    }

    #[test]
    fn compute_diff_should_detect_added_and_removed_dependencies() {
        let path = make_asset_path("/Game/T_Rock");
        let dep_a = make_asset_path("/Game/A");
        let dep_b = make_asset_path("/Game/B");
        let dep_c = make_asset_path("/Game/C");

        let old = make_meta(
            path.clone(), // clone required: path reused for new meta
            AssetType::Texture2D,
            vec![dep_a.clone(), dep_b.clone()], // clone required: deps reused below
            None,
        );
        let new = make_meta(
            path.clone(), // clone required: path passed to compute_diff separately
            AssetType::Texture2D,
            vec![dep_b, dep_c.clone()], // clone required: dep_c checked in assertion
            None,
        );

        let diff = compute_diff(path, old, new);

        assert_eq!(diff.deps_added, vec![dep_c]);
        assert_eq!(diff.deps_removed, vec![dep_a]);
        assert!(diff.type_changed.is_none());
        assert!(diff.metrics_delta.is_none());
    }

    #[test]
    fn compute_diff_should_detect_type_change() {
        let path = make_asset_path("/Game/BP_Enemy");

        let old = make_meta(
            path.clone(), // clone required: path reused
            AssetType::Blueprint,
            vec![],
            None,
        );
        let new = make_meta(
            path.clone(), // clone required: path reused
            AssetType::StaticMesh,
            vec![],
            None,
        );

        let diff = compute_diff(path, old, new);

        assert_eq!(
            diff.type_changed,
            Some((AssetType::Blueprint, AssetType::StaticMesh))
        );
    }

    #[test]
    fn compute_diff_should_compute_node_count_delta_for_blueprint_assets() {
        let path = make_asset_path("/Game/BP_Enemy");

        let old = make_meta(
            path.clone(), // clone required: path reused
            AssetType::Blueprint,
            vec![],
            Some(uasset_lens_scanner::BlueprintMetrics {
                node_count: 3,
                event_tick_count: 0,
                cast_count: 0,
                dependency_depth: 1,
            }),
        );
        let new = make_meta(
            path.clone(), // clone required: path reused
            AssetType::Blueprint,
            vec![],
            Some(uasset_lens_scanner::BlueprintMetrics {
                node_count: 5,
                event_tick_count: 1,
                cast_count: 0,
                dependency_depth: 1,
            }),
        );

        let diff = compute_diff(path, old, new);

        assert_eq!(
            diff.metrics_delta,
            Some(MetricsDelta {
                node_count_delta: 2,
                event_tick_count_delta: 1,
            })
        );
    }

    #[test]
    fn compute_diff_should_return_none_metrics_delta_when_either_asset_lacks_metrics() {
        let path = make_asset_path("/Game/BP_Enemy");

        let old = make_meta(
            path.clone(), // clone required: path reused
            AssetType::Blueprint,
            vec![],
            None,
        );
        let new = make_meta(
            path.clone(), // clone required: path reused
            AssetType::Blueprint,
            vec![],
            Some(uasset_lens_scanner::BlueprintMetrics {
                node_count: 5,
                event_tick_count: 1,
                cast_count: 0,
                dependency_depth: 1,
            }),
        );

        let diff = compute_diff(path, old, new);

        assert!(diff.metrics_delta.is_none());
    }
}
