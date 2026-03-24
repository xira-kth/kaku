use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    target: PathBuf,
    last_reload: Instant,
}

impl FileWatcher {
    pub fn new(path: &Path) -> notify::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(sender, Config::default())?;
        watcher.watch(path, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver,
            target: path.to_path_buf(),
            last_reload: Instant::now(),
        })
    }

    pub fn has_changes(&mut self) -> bool {
        let mut changed = false;

        while let Ok(event) = self.receiver.try_recv() {
            if let Ok(event) = event {
                changed |= event.paths.iter().any(|path| path == &self.target);
            }
        }

        if changed && self.last_reload.elapsed() >= Duration::from_millis(200) {
            self.last_reload = Instant::now();
            return true;
        }

        false
    }
}
