use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;

pub fn handle_watch(
    project_dir: &Path,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
) -> anyhow::Result<i32> {
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    eprintln!("Running initial scan...");
    super::scan::handle_scan(
        project_dir,
        db_path,
        &crate::FormatKind::Text,
        cfg,
        &super::scan::ScanOptions {
            full_scan: false,
            diff: false,
            yes: true, // non-interactive startup, auto-remove stale records
            save_baseline: None,
            diff_from: None,
            quiet: false,
            suppress_report: false,
            no_color: false,
            no_progress: true, // keep the watch session's own output clean
        },
    )?;

    let content_root = crate::resolve_content_root(project_dir);
    let db = crate::open_db(db_path)?;
    // Re-load config from disk so the watch loop picks up any file edits during the session.
    let config = crate::config::load_config(project_dir);

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone(); // clone required: ctrlc handler must be 'static + Send
        if let Err(e) = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed)) {
            // MultipleHandlers occurs only in tests; system errors are non-fatal but logged
            tracing::warn!(error = %e, "Failed to install Ctrl+C handler; use SIGTERM to stop");
        }
    }

    let watcher = uasset_lens_watcher::Watcher::new(project_dir)
        .context("Failed to start file system watcher")?;

    let mut session =
        uasset_lens_watcher::WatchSession::new(db, content_root, config.scan.external_roots);
    session
        .init()
        .context("Failed to initialize watch session")?;

    eprintln!("Watching for changes. Press Ctrl+C to stop.");

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(batch) = watcher.next_batch_timeout(Duration::from_millis(200)) {
            print_batch_header(&batch);
            session
                .process_batch(&batch, &mut std::io::stdout())
                .context("Watch session error")?;
        }
    }

    eprintln!("Watch stopped.");
    Ok(0)
}

fn print_batch_header(batch: &[uasset_lens_watcher::WatchEvent]) {
    let ts = utc_hms();
    for event in batch {
        let kind = match event.kind {
            uasset_lens_watcher::WatchEventKind::Changed => "Changed",
            uasset_lens_watcher::WatchEventKind::Created => "Created",
            uasset_lens_watcher::WatchEventKind::Deleted => "Deleted",
        };
        for path in &event.paths {
            println!("[{ts}] {kind}: {}", path.display());
        }
    }
}

// Returns UTC time — no chrono dependency, local timezone is not available in std.
fn utc_hms() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default() // only fails if system clock is before 1970; safe default is 0s
        .as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        (s % 86400) / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;

    #[test]
    fn handle_watch_should_return_err_when_project_dir_is_not_watchable() {
        let (dir, db_path) = test_db_in_tempdir("watch_nonexist");
        // project_dir must not exist; db_path stays inside dir so create_dir_all in
        // handle_watch recreates dir but not proj, keeping project_dir non-existent.
        let proj = dir.join("proj");
        let result = handle_watch(&proj, &db_path, &Default::default());
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
