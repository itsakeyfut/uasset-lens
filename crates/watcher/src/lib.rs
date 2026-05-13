use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode};
// Bring the Watcher trait into scope for .watch() without naming it,
// avoiding a conflict with our own `Watcher` struct.
use notify::Watcher as _;

/// The kind of file system change detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    Changed,
    Created,
    Deleted,
}

/// A debounced batch of file system events sharing the same kind.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub paths: Vec<PathBuf>,
}

/// Error type for watcher operations.
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("file system watcher error: {0}")]
    Notify(#[from] notify::Error),
}

const DEBOUNCE: Duration = Duration::from_millis(300);

/// Cross-platform file system watcher with 300 ms debounce for `.uasset` / `.umap` files.
pub struct Watcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<Vec<WatchEvent>>,
}

impl Watcher {
    /// Start watching `project_dir` recursively. Returns an error if the OS
    /// watcher cannot be initialised (e.g. too many open files, bad path).
    pub fn new(project_dir: &Path) -> Result<Self, WatcherError> {
        let (raw_tx, raw_rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let (out_tx, out_rx) = mpsc::channel::<Vec<WatchEvent>>();

        let mut inner = RecommendedWatcher::new(raw_tx, Config::default())?;
        inner.watch(project_dir, RecursiveMode::Recursive)?;
        tracing::info!(path = %project_dir.display(), "watching project directory");

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, DEBOUNCE));

        Ok(Self {
            _watcher: inner,
            rx: out_rx,
        })
    }

    /// Block until the next debounced batch of events is available.
    /// Returns `None` when the watcher has been shut down (all senders dropped).
    pub fn next_batch(&self) -> Option<Vec<WatchEvent>> {
        self.rx.recv().ok()
    }
}

/// Collect raw notify events, apply debounce, and forward batches on `out_tx`.
/// Exposed as `pub(crate)` so unit tests can inject synthetic events directly.
pub(crate) fn run_debounce(
    raw_rx: Receiver<notify::Result<notify::Event>>,
    out_tx: Sender<Vec<WatchEvent>>,
    debounce: Duration,
) {
    loop {
        let first = match raw_rx.recv() {
            Ok(evt) => evt,
            Err(_) => {
                tracing::debug!("watcher dropped; debounce thread exiting");
                return;
            }
        };

        let mut acc: HashMap<PathBuf, WatchEventKind> = HashMap::new();
        accumulate(&mut acc, first);

        // Drain additional events that arrive within the debounce window.
        let deadline = Instant::now() + debounce;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match raw_rx.recv_timeout(remaining) {
                Ok(evt) => accumulate(&mut acc, evt),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !acc.is_empty() {
                        tracing::debug!(count = acc.len(), "emitting debounced batch");
                        let _ = out_tx.send(to_batch(acc));
                    }
                    tracing::debug!("watcher dropped; debounce thread exiting");
                    return;
                }
            }
        }

        if !acc.is_empty() {
            tracing::debug!(count = acc.len(), "emitting debounced batch");
            if out_tx.send(to_batch(acc)).is_err() {
                tracing::debug!("batch consumer dropped; debounce thread exiting");
                return;
            }
        }
    }
}

fn accumulate(acc: &mut HashMap<PathBuf, WatchEventKind>, raw: notify::Result<notify::Event>) {
    let evt = match raw {
        Ok(e) => e,
        Err(_) => return,
    };
    let Some(kind) = to_kind(&evt.kind) else {
        return;
    };
    for path in evt.paths {
        if is_relevant(&path) {
            acc.insert(path, kind.clone()); // clone required: each path gets its own kind entry
        }
    }
}

fn is_relevant(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("uasset") | Some("umap")
    )
}

fn to_kind(kind: &notify::EventKind) -> Option<WatchEventKind> {
    match kind {
        notify::EventKind::Create(_) => Some(WatchEventKind::Created),
        notify::EventKind::Modify(_) => Some(WatchEventKind::Changed),
        notify::EventKind::Remove(_) => Some(WatchEventKind::Deleted),
        _ => None,
    }
}

/// Flush the accumulator into a sorted, grouped `Vec<WatchEvent>`.
fn to_batch(acc: HashMap<PathBuf, WatchEventKind>) -> Vec<WatchEvent> {
    let mut changed: Vec<PathBuf> = Vec::new();
    let mut created: Vec<PathBuf> = Vec::new();
    let mut deleted: Vec<PathBuf> = Vec::new();

    for (path, kind) in acc {
        match kind {
            WatchEventKind::Changed => changed.push(path),
            WatchEventKind::Created => created.push(path),
            WatchEventKind::Deleted => deleted.push(path),
        }
    }

    let mut result = Vec::new();
    if !changed.is_empty() {
        changed.sort();
        result.push(WatchEvent {
            kind: WatchEventKind::Changed,
            paths: changed,
        });
    }
    if !created.is_empty() {
        created.sort();
        result.push(WatchEvent {
            kind: WatchEventKind::Created,
            paths: created,
        });
    }
    if !deleted.is_empty() {
        deleted.sort();
        result.push(WatchEvent {
            kind: WatchEventKind::Deleted,
            paths: deleted,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // Short debounce so tests don't wait 300 ms.
    const SHORT: Duration = Duration::from_millis(50);

    fn modify_event(path: PathBuf) -> notify::Result<notify::Event> {
        Ok(
            notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Any))
                .add_path(path),
        )
    }

    fn create_event(path: PathBuf) -> notify::Result<notify::Event> {
        Ok(
            notify::Event::new(notify::EventKind::Create(notify::event::CreateKind::Any))
                .add_path(path),
        )
    }

    fn remove_event(path: PathBuf) -> notify::Result<notify::Event> {
        Ok(
            notify::Event::new(notify::EventKind::Remove(notify::event::RemoveKind::Any))
                .add_path(path),
        )
    }

    #[test]
    fn debounce_should_deduplicate_events_for_same_path_within_window() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, SHORT));

        let path = PathBuf::from("/Game/T_Rock.uasset");
        for _ in 0..3 {
            raw_tx.send(modify_event(path.clone())).unwrap();
        }
        drop(raw_tx);

        let batch = out_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("expected a debounced batch");
        assert_eq!(
            batch.len(),
            1,
            "all modify events collapse to one WatchEvent"
        );
        assert_eq!(batch[0].kind, WatchEventKind::Changed);
        assert_eq!(batch[0].paths.len(), 1, "deduplicated to a single path");
        assert_eq!(batch[0].paths[0], path);
    }

    #[test]
    fn debounce_should_exclude_non_uasset_and_non_umap_file_changes() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, SHORT));

        raw_tx
            .send(modify_event(PathBuf::from("/Game/icon.png")))
            .unwrap();
        raw_tx
            .send(modify_event(PathBuf::from("/Game/config.ini")))
            .unwrap();
        drop(raw_tx);

        assert!(
            out_rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "non-.uasset/.umap files must not produce a batch"
        );
    }

    #[test]
    fn debounce_should_include_umap_files() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, SHORT));

        let path = PathBuf::from("/Game/L_Main.umap");
        raw_tx.send(create_event(path.clone())).unwrap();
        drop(raw_tx);

        let batch = out_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("expected a batch for .umap file");
        assert_eq!(batch[0].kind, WatchEventKind::Created);
        assert_eq!(batch[0].paths[0], path);
    }

    #[test]
    fn debounce_should_group_multiple_paths_of_same_kind_into_one_event() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, SHORT));

        let path_a = PathBuf::from("/Game/A/T_Rock.uasset");
        let path_b = PathBuf::from("/Game/B/T_Grass.uasset");
        raw_tx.send(modify_event(path_a.clone())).unwrap();
        raw_tx.send(modify_event(path_b.clone())).unwrap();
        drop(raw_tx);

        let batch = out_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("expected a batch");
        assert_eq!(
            batch.len(),
            1,
            "two Changed paths merge into one WatchEvent"
        );
        assert_eq!(batch[0].paths.len(), 2);
        assert!(batch[0].paths.contains(&path_a));
        assert!(batch[0].paths.contains(&path_b));
    }

    #[test]
    fn debounce_should_preserve_deleted_event_kind() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();

        std::thread::spawn(move || run_debounce(raw_rx, out_tx, SHORT));

        let path = PathBuf::from("/Game/T_Old.uasset");
        raw_tx.send(remove_event(path.clone())).unwrap();
        drop(raw_tx);

        let batch = out_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("expected a batch");
        assert_eq!(batch[0].kind, WatchEventKind::Deleted);
        assert_eq!(batch[0].paths[0], path);
    }
}
