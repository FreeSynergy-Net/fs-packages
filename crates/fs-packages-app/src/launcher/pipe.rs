//! The pipe from a started program into its log pump: the log file, the pipe and the pump process (a part of [`super::ProcessLauncher`]).

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::launch_log::{create_private_dir, log_file, open_fresh};
use crate::launch_pump::PUMP_ARG;
use crate::launch_watch::LaunchReport;

use super::ProcessLauncher;

/// The log, the output ends of the program and the pump.
pub(super) type Logged = (Option<PathBuf>, Stdio, Stdio, Option<Child>);
/// The log, the two ends of the pipe into the pump and the pump.
type Made = (PathBuf, std::io::PipeWriter, std::io::PipeWriter, Child);

impl ProcessLauncher {
    /// Makes the log of `command` and the pipe into it. Returns the log, the two ends the program writes to and the pump. When the log or
    /// the pump cannot be made, the cause is queued as a report (never silent) and the program gets nothing attached.
    pub(super) fn open_log(&self, command: &str) -> Logged {
        match self.make_log(command) {
            Ok((path, out, err, pump)) => (Some(path), out.into(), err.into(), Some(pump)),
            Err(cause) => {
                self.reports.push(LaunchReport::LogUnwritable {
                    command: command.to_owned(),
                    cause,
                });
                if let Some(redraw) = &self.redraw {
                    redraw();
                }
                (None, Stdio::null(), Stdio::null(), None)
            }
        }
    }

    fn make_log(&self, command: &str) -> Result<Made, String> {
        let dir = self
            .state
            .as_ref()
            .ok_or_else(|| "no home folder (HOME is not set)".to_owned())?;
        create_private_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = log_file(dir, command);
        let at = |e: std::io::Error| format!("{}: {e}", path.display());
        // Made and wiped here (without following a link); the pump fills it.
        drop(open_fresh(&path).map_err(at)?);
        let exe = self
            .pump
            .as_ref()
            .ok_or_else(|| "the log pump (this program) was not found".to_owned())?;
        let (reader, writer) = std::io::pipe().map_err(at)?;
        let mut pump = Command::new(exe);
        pump.arg(PUMP_ARG)
            .arg(&path)
            .stdin(reader)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            pump.process_group(0);
        }
        let pump = pump
            .spawn()
            .map_err(|e| format!("log pump {}: {e}", exe.display()))?;
        let err = writer.try_clone().map_err(at)?;
        Ok((path, writer, err, pump))
    }
}
