//! Pattern: Observer (one thread per started program watches it; what the person should know is a [`LaunchReport`] in a queue that the
//! window empties at its next drawing).
//!
//! Three things are reported: a program that ends with a failure within the observation time (with the last lines of its log); one that
//! ends within that time WITHOUT a failure (exit code 0: "ended at once", a starter script that hands over to another process looks like
//! this too, so it is a hint and not an error); and one that still runs after [`HANG_AFTER`] without a sign of life
//! ([`crate::launch_life`]), which the person can stop (only this child, by its handle: never by name, never another process). A program
//! that runs and shows its sign, or ends later, is not reported.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::launch_life::SignProbe;
use crate::launch_log::{SHOWN_LINES, last_lines};

/// How long a started program is watched for an early end.
pub const OBSERVE: Duration = Duration::from_secs(3);
/// The check interval while the program is watched closely.
const FAST: Duration = Duration::from_millis(50);
/// After this long a program that is still running and shows no sign of life is reported.
pub const HANG_AFTER: Duration = Duration::from_secs(8);
/// The check interval afterwards.
const SLOW: Duration = Duration::from_secs(2);
/// How long the end of the program waits for the pump to write the last lines before the log is read.
const PUMP_WAIT: Duration = Duration::from_secs(1);

/// Something the person must be told about a started program. Data only; the window finds the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchReport {
    /// The program ended with a failure within the observation time.
    Failed {
        /// The command that was started.
        command: String,
        /// The exit code; `None` when a signal ended it.
        code: Option<i32>,
        /// The last lines of its log (may be empty).
        lines: Vec<String>,
    },
    /// The program ended within the observation time with exit code 0.
    EndedAtOnce {
        /// The command that was started.
        command: String,
        /// The last lines of its log (may be empty).
        lines: Vec<String>,
    },
    /// The program still runs but shows no sign of life (no window) since `seconds`.
    NoSignOfLife {
        /// The command that was started.
        command: String,
        /// How long it has run.
        seconds: u64,
    },
    /// The program that had no sign of life shows one now.
    SignOfLife {
        /// The command that was started.
        command: String,
    },
    /// The log could not be written; the program was started without one.
    LogUnwritable {
        /// The command that was started.
        command: String,
        /// The cause, as the system says it.
        cause: String,
    },
}

impl LaunchReport {
    /// The command the report is about.
    #[must_use]
    pub fn command(&self) -> &str {
        match self {
            Self::Failed { command, .. }
            | Self::EndedAtOnce { command, .. }
            | Self::NoSignOfLife { command, .. }
            | Self::SignOfLife { command }
            | Self::LogUnwritable { command, .. } => command,
        }
    }

    /// The stable code of the report (the words are found from it and its parameters, in the language of the person).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Failed { .. } => "launch-failed",
            Self::EndedAtOnce { .. } => "launch-ended-at-once",
            Self::NoSignOfLife { .. } => "launch-no-sign-of-life",
            Self::SignOfLife { .. } => "launch-sign-of-life",
            Self::LogUnwritable { .. } => "launch-log-unwritable",
        }
    }
}

/// The queue between the watching threads and the window.
#[derive(Debug, Clone, Default)]
pub struct Reports(Arc<Mutex<Vec<LaunchReport>>>);

impl Reports {
    /// Puts a report into the queue.
    pub fn push(&self, report: LaunchReport) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(report);
    }

    /// Takes all reports out.
    #[must_use]
    pub fn take(&self) -> Vec<LaunchReport> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

type Shared = Arc<Mutex<Child>>;

/// The children the store started and that still run, by command: the only processes the store ever stops.
#[derive(Clone, Default)]
pub struct Running(Arc<Mutex<HashMap<String, Shared>>>);

impl Running {
    fn add(&self, command: &str, child: &Shared) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(command.to_owned(), Arc::clone(child));
    }

    fn remove(&self, command: &str, child: &Shared) {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if map.get(command).is_some_and(|c| Arc::ptr_eq(c, child)) {
            map.remove(command);
        }
    }

    /// Ends the child the store started as `command`. True when there was one to end.
    pub fn stop(&self, command: &str) -> bool {
        let child = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(command)
            .cloned();
        child.is_some_and(|c| {
            c.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .kill()
                .is_ok()
        })
    }
}

/// What a watching thread needs to know.
pub(crate) struct Watch {
    /// The command, as told in a report.
    pub command: String,
    /// Its log, when there is one.
    pub log: Option<PathBuf>,
    /// The log pump that belongs to it (reaped here when it ends).
    pub pump: Option<Child>,
    /// Where the reports go.
    pub reports: Reports,
    /// The children that can be stopped.
    pub running: Running,
    /// How long an end counts as early.
    pub observe: Duration,
    /// After this long a program without a sign of life is reported.
    pub hang_after: Duration,
    /// Tells whether the program shows a sign of life.
    pub probe: SignProbe,
    /// Called after a report was queued (the window draws again).
    pub redraw: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Watch {
    fn tell(&self, report: LaunchReport) {
        self.reports.push(report);
        if let Some(redraw) = &self.redraw {
            redraw();
        }
    }
}

/// Watches `child` on a thread of its own until it ends (so it never stays a zombie), and then its pump.
pub(crate) fn watch(child: Child, mut watch: Watch) {
    let pid = child.id();
    let child: Shared = Arc::new(Mutex::new(child));
    watch.running.add(&watch.command, &child);
    std::thread::spawn(move || {
        let began = Instant::now();
        let mut probing = true;
        let mut hint = false;
        loop {
            let status = child
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .try_wait();
            match status {
                Ok(Some(status)) => {
                    watch.running.remove(&watch.command, &child);
                    let early = began.elapsed() <= watch.observe;
                    finish(&mut watch, status, early);
                    return;
                }
                Ok(None) => {}
                Err(_) => {
                    watch.running.remove(&watch.command, &child);
                    return;
                }
            }
            let age = began.elapsed();
            if probing && age >= watch.hang_after {
                match ((watch.probe)(pid), hint) {
                    (Some(false), false) => {
                        hint = true;
                        watch.tell(LaunchReport::NoSignOfLife {
                            command: watch.command.clone(),
                            seconds: age.as_secs(),
                        });
                    }
                    (Some(false), true) => {}
                    (Some(true), true) => {
                        probing = false;
                        watch.tell(LaunchReport::SignOfLife {
                            command: watch.command.clone(),
                        });
                    }
                    _ => probing = false,
                }
            }
            std::thread::sleep(if age <= watch.observe { FAST } else { SLOW });
        }
    });
}

fn finish(watch: &mut Watch, status: ExitStatus, early: bool) {
    // The pump writes what the program wrote; give it the time to be done before the log is read.
    let waited = Instant::now();
    if let Some(pump) = watch.pump.as_mut() {
        while pump.try_wait().is_ok_and(|s| s.is_none()) && waited.elapsed() < PUMP_WAIT {
            std::thread::sleep(FAST / 5);
        }
    }
    if early {
        let lines = watch
            .log
            .as_ref()
            .map(|log| last_lines(log, SHOWN_LINES))
            .unwrap_or_default();
        let command = watch.command.clone();
        watch.tell(if status.success() {
            LaunchReport::EndedAtOnce { command, lines }
        } else {
            LaunchReport::Failed {
                command,
                code: status.code(),
                lines,
            }
        });
    }
    // A program that started others may leave the pump running for as long as they write: reap it when it ends.
    if let Some(mut pump) = watch.pump.take() {
        while pump.try_wait().is_ok_and(|s| s.is_none()) {
            std::thread::sleep(SLOW);
        }
    }
}
