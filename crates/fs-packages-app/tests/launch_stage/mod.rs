//! Shared helpers of the launcher tests: stub programs in a folder of the test target, one lock, and a wait for reports.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::{Launcher, ProcessLauncher};

static STUBS: Mutex<()> = Mutex::new(());

pub fn guard() -> std::sync::MutexGuard<'static, ()> {
    STUBS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct Stage {
    pub dir: PathBuf,
}

pub fn stage(name: &str) -> Stage {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("launch-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    let stage = Stage { dir };
    // The stand-in for the log pump: `<pump> log-pump <log>` copies its input into the log.
    stage.stub("log-pump-stub", "exec cat >> \"$2\"");
    stage
}

impl Stage {
    pub fn stub(&self, name: &str, body: &str) {
        use std::io::Write;
        let path = self.dir.join("bin").join(name);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn state(&self) -> PathBuf {
        self.dir.join("state").join("fs-packages")
    }

    /// A launcher with the stub pump and a probe that says `answer` for every program.
    pub fn launcher(&self, observe: Duration, answer: Option<bool>) -> ProcessLauncher {
        ProcessLauncher::new(self.dir.join("bin"))
            .with_pump(self.dir.join("bin").join("log-pump-stub"))
            .with_probe(Arc::new(move |_| answer))
            .with_hang_after(Duration::from_millis(400))
            .with_state_dir(self.state())
            .with_observe(observe)
    }
}

pub fn wait_report(l: &ProcessLauncher) -> Vec<LaunchReport> {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        let r = l.take_reports();
        if !r.is_empty() {
            return r;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Vec::new()
}

/// True while the process `pid` exists (a reaped child is gone; a zombie still counts as there).
pub fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

pub fn wait_until(what: impl Fn() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    what()
}

pub fn read_pid(path: &Path) -> u32 {
    assert!(wait_until(
        || fs::read_to_string(path).is_ok_and(|t| !t.is_empty())
    ));
    fs::read_to_string(path).unwrap().trim().parse().unwrap()
}
