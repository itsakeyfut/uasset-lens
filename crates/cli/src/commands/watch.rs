use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;

pub fn handle_watch(project_dir: &Path, db_path: &Path) -> anyhow::Result<i32> {
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    eprintln!("Running initial scan...");
    super::scan::handle_scan(
        project_dir,
        false,
        false,
        db_path,
        &crate::FormatKind::Text,
        true, // yes=true: non-interactive startup, auto-remove stale records
        None,
        None,
    )?;

    let content_root = crate::resolve_content_root(project_dir);
    let db = crate::open_db(db_path)?;

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone(); // clone required: ctrlc handler must be 'static + Send
        if let Err(e) = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed)) {
            // MultipleHandlers occurs only in tests; system errors are non-fatal but logged
            tracing::warn!(error = %e, "Failed to install Ctrl+C handler; use SIGTERM to stop");
        }
    }

    let watcher =
        watcher::Watcher::new(project_dir).context("Failed to start file system watcher")?;

    let mut session = watcher::WatchSession::new(db, content_root);
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

fn print_batch_header(batch: &[watcher::WatchEvent]) {
    let ts = utc_hms();
    for event in batch {
        let kind = match event.kind {
            watcher::WatchEventKind::Changed => "Changed",
            watcher::WatchEventKind::Created => "Created",
            watcher::WatchEventKind::Deleted => "Deleted",
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

    #[test]
    fn handle_watch_should_return_err_when_project_dir_is_not_watchable() {
        let dir = std::env::temp_dir().join(format!("watch_nonexist_{}", std::process::id()));
        let db_path =
            std::env::temp_dir().join(format!("watch_nonexist_{}.db", std::process::id()));
        // Watcher::new() errors on a non-existent path
        let result = handle_watch(&dir, &db_path);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&db_path);
    }
}
