use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    target: PathBuf,
    target_name: Option<String>,
    last_reload: Instant,
}

impl FileWatcher {
    pub fn new(path: &Path) -> notify::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(sender, Config::default())?;
        let watched_path = path.parent().unwrap_or_else(|| Path::new("."));
        watcher.watch(watched_path, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver,
            target: path.to_path_buf(),
            target_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            last_reload: Instant::now(),
        })
    }

    pub fn has_changes(&mut self) -> bool {
        let mut changed = false;

        while let Ok(event) = self.receiver.try_recv() {
            if let Ok(event) = event {
                changed |= event.paths.iter().any(|path| self.matches_target(path));
            }
        }

        if changed && self.last_reload.elapsed() >= Duration::from_millis(200) {
            self.last_reload = Instant::now();
            return true;
        }

        false
    }

    fn matches_target(&self, path: &Path) -> bool {
        if path == self.target {
            return true;
        }

        if let (Ok(left), Ok(right)) = (path.canonicalize(), self.target.canonicalize()) {
            if left == right {
                return true;
            }
        }

        self.target_name
            .as_ref()
            .zip(path.file_name())
            .is_some_and(|(expected, actual)| expected == &actual.to_string_lossy())
    }
}
