//! Pattern: Observer (one thread per started program watches it for the first seconds and keeps its log short; what went wrong is a
//! [`LaunchReport`] in a queue that the window empties at its next drawing).
//!
//! A program that ends with a failure within the observation time is reported with the last lines of its log. A program that keeps
//! running, or ends cleanly (also early: a starter script that hands over to another process ends with 0), is not reported.

use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::launch_log::{SHOWN_LINES, last_lines, trim};

/// How long a started program is watched for an early end.
pub const OBSERVE: Duration = Duration::from_secs(3);
/// The check interval while the program is watched closely.
const FAST: Duration = Duration::from_millis(50);
/// The check interval afterwards (the log is kept short at each check).
const SLOW: Duration = Duration::from_secs(2);

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
    /// The log could not be written; the program was started without one.
    LogUnwritable {
        /// The command that was started.
        command: String,
        /// The cause, as the system says it.
        cause: String,
    },
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

/// What a watching thread needs to know.
pub(crate) struct Watch {
    /// The command, as told in a report.
    pub command: String,
    /// Its log, when there is one.
    pub log: Option<PathBuf>,
    /// Where the reports go.
    pub reports: Reports,
    /// How long an end counts as early.
    pub observe: Duration,
    /// Called after a report was queued (the window draws again).
    pub redraw: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// Watches `child` on a thread of its own until it ends (so it never stays a zombie).
pub(crate) fn watch(mut child: Child, watch: Watch) {
    std::thread::spawn(move || {
        let began = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    finish(&watch, status, began.elapsed() <= watch.observe);
                    return;
                }
                Ok(None) => {}
                Err(_) => return,
            }
            let early = began.elapsed() <= watch.observe;
            if let Some(log) = watch.log.as_ref().filter(|_| !early) {
                let _ = trim(log);
            }
            std::thread::sleep(if early { FAST } else { SLOW });
        }
    });
}

fn finish(watch: &Watch, status: ExitStatus, early: bool) {
    if let Some(log) = &watch.log {
        let _ = trim(log);
    }
    if early && !status.success() {
        let lines = watch
            .log
            .as_ref()
            .map(|log| last_lines(log, SHOWN_LINES))
            .unwrap_or_default();
        watch.reports.push(LaunchReport::Failed {
            command: watch.command.clone(),
            code: status.code(),
            lines,
        });
        if let Some(redraw) = &watch.redraw {
            redraw();
        }
    }
}
