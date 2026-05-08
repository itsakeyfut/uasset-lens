# `apps/uasset-lens-desktop` — GUI app skeleton and project loading

## Summary

Create the `uasset-lens-desktop` binary crate using `eframe`/`egui`, implementing
project directory selection, scan execution, and a progress indicator.
Complete when the app window opens, a project can be selected, and a scan runs
with a progress bar visible.

## Design Notes

**Crate:** `apps/uasset-lens-desktop/` — binary crate using `eframe` + `egui`.

Add to `[workspace.dependencies]`:
- `eframe` (with `default_features = false`, feature `"default_fonts"`)
- `egui`

**App state machine:**

```rust
enum AppState {
    Welcome,                  // no project selected
    Scanning { progress: f32 },
    Ready(ProjectData),       // scan complete, dashboard available
    Error(String),
}

struct ProjectData {
    assets:     Vec<AssetRecord>,
    dead:       Vec<AssetPath>,
    cycles:     Vec<Vec<AssetPath>>,
    graph:      DependencyGraph,
}
```

**Welcome screen:**
- "Open Project" button → use `rfd` crate for a native file dialog (folder selection)
- Selected path displayed below the button

**Scanning:**
- Run scan in a background thread (`std::thread::spawn`)
- Send progress updates via `std::sync::mpsc` channel
- Show `egui::ProgressBar` during scan

**Thread safety:** the `egui` app runs on the main thread; the scan thread communicates
results back through a channel, not shared mutable state.

## Requirements

- [ ] Create `apps/uasset-lens-desktop` binary crate with `eframe` + `egui`
- [ ] Add `eframe`, `egui`, `rfd` to `[workspace.dependencies]`
- [ ] Implement `AppState` enum and `App` struct implementing `eframe::App`
- [ ] Welcome screen: "Open Project" button + native folder dialog via `rfd`
- [ ] Scanning state: spawn scan thread, receive progress via `mpsc`, display `ProgressBar`
- [ ] Transition to `AppState::Ready` when scan completes
- [ ] `cargo build -p uasset-lens-desktop --release` succeeds on Windows

## Related

- Next: #7 — summary panel and dead assets list
- Docs: `docs/roadmap/phase6/ROADMAP.md` — Task 4-1
