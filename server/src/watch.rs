//! Watches the music folder, so a song added (by Lidarr, a download, or you copying files in) or removed shows up
//! within a few seconds. It's the main way the library stays current: the timed scan (VH_RESCAN_MINUTES) remains as
//! a safety net for folders that can't report changes (some network shares), and Lidarr no longer needs a webhook.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub struct Watch {
    tx: UnboundedSender<()>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    live: AtomicBool,
}

impl Watch {
    pub fn new() -> (Watch, UnboundedReceiver<()>) {
        let (tx, rx) = unbounded_channel();
        (Watch { tx, watcher: Mutex::new(None), live: AtomicBool::new(false) }, rx)
    }

    /// Is the folder being watched right now?
    pub fn live(&self) -> bool {
        self.live.load(Ordering::Relaxed)
    }

    /// (Re)start watching a folder. Returns false when the system can't watch it (then the timed scan covers it).
    pub fn watch(&self, dir: &Path) -> bool {
        let tx = self.tx.clone();
        let made = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                // opening or reading a file isn't a change
                if !matches!(ev.kind, EventKind::Access(_)) {
                    let _ = tx.send(());
                }
            }
        });
        let ok = match made {
            Ok(mut w) => {
                let ok = w.watch(dir, RecursiveMode::Recursive).is_ok();
                *self.watcher.lock().unwrap() = if ok { Some(w) } else { None };
                ok
            }
            Err(_) => { *self.watcher.lock().unwrap() = None; false }
        };
        self.live.store(ok, Ordering::Relaxed);
        ok
    }
}
