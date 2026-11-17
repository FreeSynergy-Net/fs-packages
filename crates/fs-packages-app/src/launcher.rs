//! Pattern: Strategy / Port (starting another program sits behind a trait, so the model is tested with a fake).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use crate::launch_life::{SignProbe, system_probe};
use crate::launch_log::{log_file, state_dir};
use crate::launch_watch::{HANG_AFTER, LaunchReport, OBSERVE, Reports, Running, Watch, watch};

mod pipe;

/// Starts programs as processes of their own.
pub trait Launcher: Send + Sync {
    /// True if `command` can be started (found on the PATH or in the store's command folder).
    fn exists(&self, command: &str) -> bool;
    /// Starts `command` so that it outlives the store. Its output goes to a log; a failure in the first seconds comes back through
    /// [`Launcher::take_reports`].
    ///
    /// # Errors
    /// A developer text when it could not be started.
    fn start(&self, command: &str) -> Result<(), String>;
    /// What happened to started programs since the last call (an early failure, a log that could not be written). Empty by default.
    fn take_reports(&self) -> Vec<LaunchReport> {
        Vec::new()
    }
    /// The log of `command`, when it exists. `None` by default.
    fn log_path(&self, _command: &str) -> Option<PathBuf> {
        None
    }
    /// Ends the process the store itself started as `command` (by its handle, never by name). True when there was one. False by default.
    fn stop(&self, _command: &str) -> bool {
        false
    }
    /// Opens a web address with the system's opener (`xdg-open`) as a process of its own.
    ///
    /// # Errors
    /// A developer text when it could not be started.
    fn open_url(&self, url: &str) -> Result<(), String>;
}

/// The real launcher: a program is a direct child in a process group of its own (it survives the store and its terminal), its output goes
/// through a pipe to the log pump ([`crate::launch_pump`]), which keeps `launch-<name>.log` in the state folder at 64 KB at most for as
/// long as the program writes, and a thread watches the program ([`crate::launch_watch`]). The opener for addresses stays `setsid -f`
/// when available.
#[derive(Clone)]
pub struct ProcessLauncher {
    extra: PathBuf,
    state: Option<PathBuf>,
    reports: Reports,
    running: Running,
    observe: Duration,
    hang_after: Duration,
    probe: SignProbe,
    pump: Option<PathBuf>,
    redraw: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for ProcessLauncher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessLauncher")
            .field("extra", &self.extra)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ProcessLauncher {
    /// Also looks in `extra` (the folder the store links commands into), for it may not be on the PATH. The logs go to
    /// `$XDG_STATE_HOME/fs-packages` (else `~/.local/state/fs-packages`).
    #[must_use]
    pub fn new(extra: PathBuf) -> Self {
        Self {
            extra,
            state: state_dir(),
            reports: Reports::default(),
            running: Running::default(),
            observe: OBSERVE,
            hang_after: HANG_AFTER,
            probe: system_probe(),
            pump: pump_exe(),
            redraw: None,
        }
    }

    /// The logs go to `dir` (tests; the folder is made when the first program starts).
    #[must_use]
    pub fn with_state_dir(mut self, dir: PathBuf) -> Self {
        self.state = Some(dir);
        self
    }

    /// How long an end counts as early (default 3 s; tests make it shorter).
    #[must_use]
    pub fn with_observe(mut self, observe: Duration) -> Self {
        self.observe = observe;
        self
    }

    /// After how long a program that shows no sign of life is reported (default 8 s; tests make it shorter).
    #[must_use]
    pub fn with_hang_after(mut self, after: Duration) -> Self {
        self.hang_after = after;
        self
    }

    /// How the launcher tells a sign of life (tests decide the answer; the default reads the loaded libraries of the child on Linux).
    #[must_use]
    pub fn with_probe(mut self, probe: SignProbe) -> Self {
        self.probe = probe;
        self
    }

    /// The program that runs as the log pump, called as `<pump> log-pump <log>` (default: this program itself).
    #[must_use]
    pub fn with_pump(mut self, pump: PathBuf) -> Self {
        self.pump = Some(pump);
        self
    }

    /// `redraw` is called after a report was queued, so the window shows it without a click.
    #[must_use]
    pub fn with_redraw(mut self, redraw: Arc<dyn Fn() + Send + Sync>) -> Self {
        self.redraw = Some(redraw);
        self
    }

    fn find(&self, command: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .chain(std::iter::once(self.extra.clone()))
            .map(|dir| dir.join(command))
            .find(|p| is_file(p))
    }
}

/// This program as the log pump. After an update of the store the running file may have been replaced: Linux then says `... (deleted)`,
/// and the file at the same path is the newer version of this program, which knows the pump as well.
fn pump_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let text = exe.to_string_lossy();
    Some(
        text.strip_suffix(" (deleted)")
            .map_or(exe.clone(), PathBuf::from),
    )
}

fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

fn detached(mut command: Command) -> Command {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// The words after `setsid`: `-f <program>` and the one argument that is asked for. Starting a program from the
/// store gives it NO argument (no `--facet`, no source): the program starts as its desktop entry would start it.
#[must_use]
pub fn setsid_words(program: &Path, arg: Option<&str>) -> Vec<std::ffi::OsString> {
    let mut words = vec!["-f".into(), program.as_os_str().to_owned()];
    words.extend(arg.map(Into::into));
    words
}

/// Runs `program arg` detached and says how it started; the child is reaped in the background.
fn run_detached(program: &Path, arg: Option<&str>) -> Result<(), String> {
    let mut setsid = Command::new("setsid");
    setsid.args(setsid_words(program, arg));
    match detached(setsid).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("setsid ended with {status}")),
        Err(_) => {
            let mut plain = Command::new(program);
            plain.args(arg);
            let mut child = detached(plain).spawn().map_err(|e| e.to_string())?;
            // Reap the child when it ends, without holding anything up.
            std::thread::spawn(move || child.wait().map(|_| ()));
            Ok(())
        }
    }
}

impl Launcher for ProcessLauncher {
    fn exists(&self, command: &str) -> bool {
        !command.contains('/') && self.find(command).is_some()
    }

    fn start(&self, command: &str) -> Result<(), String> {
        if command.contains('/') {
            return Err("not found".to_owned());
        }
        let path = self.find(command).ok_or_else(|| "not found".to_owned())?;
        let (log, out, err, pump) = self.open_log(command);
        let mut process = Command::new(&path);
        process.stdin(Stdio::null()).stdout(out).stderr(err);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.process_group(0);
        }
        let child = match process.spawn() {
            Ok(child) => child,
            Err(e) => {
                // The pump is already running: let go of the pipe (it ends at its end) and pick it up, or it stays a zombie until the store ends.
                drop(process);
                if let Some(mut pump) = pump {
                    std::thread::spawn(move || pump.wait().map(|_| ()));
                }
                return Err(e.to_string());
            }
        };
        watch(
            child,
            Watch {
                command: command.to_owned(),
                log,
                pump,
                reports: self.reports.clone(),
                running: self.running.clone(),
                observe: self.observe,
                hang_after: self.hang_after,
                probe: Arc::clone(&self.probe),
                redraw: self.redraw.clone(),
            },
        );
        Ok(())
    }

    fn take_reports(&self) -> Vec<LaunchReport> {
        self.reports.take()
    }

    fn log_path(&self, command: &str) -> Option<PathBuf> {
        let dir = self.state.as_ref()?;
        Some(log_file(dir, command)).filter(|p| is_file(p))
    }

    fn stop(&self, command: &str) -> bool {
        self.running.stop(command)
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        let opener = self
            .find("xdg-open")
            .ok_or_else(|| "xdg-open not found".to_owned())?;
        run_detached(&opener, Some(url))
    }
}
