//! Pattern: Strategy (`RunningCheck` for Linux, through `/proc`).

#[cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};

use fs_packages_core::RunningCheck;

/// Linux: a package is running when some process executes a file below `root/packages/<name>/`.
/// The calling process itself is ignored, so the store can update itself: its own old files stay on disk
/// (the switch is a symlink, nothing is overwritten) and the new version starts at the next launch.
#[derive(Debug)]
#[cfg_attr(not(unix), allow(dead_code))] // Windows: no `/proc`, the fields are not read (port open)
pub struct ProcRunningCheck {
    root: PathBuf,
    ignore: u32,
}

impl ProcRunningCheck {
    /// Looks for processes started from packages under `root`, ignoring this process.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self::ignoring(root, std::process::id())
    }

    /// Like [`Self::new`], ignoring the process `pid` instead.
    #[must_use]
    pub fn ignoring(root: &Path, pid: u32) -> Self {
        Self {
            root: root.to_owned(),
            ignore: pid,
        }
    }
}

impl RunningCheck for ProcRunningCheck {
    #[cfg(unix)]
    fn is_running(&self, name: &str) -> bool {
        self.scan(name)
    }

    /// Windows: there is no `/proc` (port open, `unsupported-on-windows{process-list}`). The trait answers only yes or no, so
    /// the safe answer is "running": an update or removal is then refused instead of replacing files of a running program.
    #[cfg(not(unix))]
    fn is_running(&self, _name: &str) -> bool {
        true
    }
}

impl ProcRunningCheck {
    #[cfg(unix)]
    fn scan(&self, name: &str) -> bool {
        let base = self.root.join("packages").join(name);
        let Ok(procs) = fs::read_dir("/proc") else {
            return false;
        };
        procs.flatten().any(|p| {
            let own =
                p.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) == Some(self.ignore);
            !own && fs::read_link(p.path().join("exe")).is_ok_and(|exe| exe.starts_with(&base))
        })
    }
}
