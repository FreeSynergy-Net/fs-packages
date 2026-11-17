//! Pattern: Command + State (what became of the programs the person started: the note on the tile of one that failed right after the
//! start, and the log the tile can open). The window calls [`StoreModel::sync_launch`] at each drawing.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::launch_watch::LaunchReport;
use crate::launcher::Launcher;
use crate::words;
use crate::words_lang::Lang;
use crate::words_launch as w;

use super::store::StoreModel;
use super::types::Problem;

/// The little state of started programs.
#[derive(Debug, Clone, Default)]
pub struct LaunchUi {
    /// Which card a started command belongs to.
    pub(super) cards: HashMap<String, String>,
    /// What to tell on a card.
    pub(super) notes: HashMap<String, LaunchReport>,
    /// The logs that exist, by card.
    pub(super) logs: HashMap<String, PathBuf>,
    /// The logs of earlier sessions were looked for.
    pub(super) scanned: bool,
}

impl StoreModel {
    /// The program of card `name` was started as `command`: an old note goes, the log is known.
    pub(super) fn note_started(&mut self, name: &str, command: &str, launcher: &dyn Launcher) {
        self.ui.launch.notes.remove(name);
        self.ui
            .launch
            .cards
            .insert(command.to_owned(), name.to_owned());
        if let Some(log) = launcher.log_path(command) {
            self.ui.launch.logs.insert(name.to_owned(), log);
        }
    }

    /// Takes what the launcher found out (an early failure, a log that could not be written) and, once, looks for the logs of earlier
    /// sessions. True when something changed.
    pub fn sync_launch(&mut self, launcher: &dyn Launcher) -> bool {
        let mut changed = false;
        if !self.ui.launch.scanned {
            self.ui.launch.scanned = true;
            let names: Vec<String> = self.installed.iter().map(|r| r.name.clone()).collect();
            for name in names {
                let Some(command) = self.program_command(&name, launcher) else {
                    continue;
                };
                if let Some(log) = launcher.log_path(&command) {
                    self.ui.launch.cards.insert(command, name.clone());
                    self.ui.launch.logs.insert(name, log);
                    changed = true;
                }
            }
        }
        for report in launcher.take_reports() {
            let command = report.command().to_owned();
            let Some(name) = self.ui.launch.cards.get(&command).cloned() else {
                continue;
            };
            if let Some(log) = launcher.log_path(&command) {
                self.ui.launch.logs.insert(name.clone(), log);
            }
            if matches!(report, LaunchReport::SignOfLife { .. }) {
                // The hint that nothing showed up is over.
                self.ui.launch.notes.remove(&name);
            } else {
                self.ui.launch.notes.insert(name, report);
            }
            changed = true;
        }
        changed
    }

    /// Stops the program of card `name` that this store started (only that process, by its handle). True when there was one.
    pub fn stop_program(&mut self, name: &str, launcher: &dyn Launcher) -> bool {
        let command = self
            .ui
            .launch
            .cards
            .iter()
            .find(|(_, card)| card.as_str() == name)
            .map(|(command, _)| command.clone());
        let stopped = command.is_some_and(|command| launcher.stop(&command));
        if stopped {
            self.ui.launch.notes.remove(name);
        }
        stopped
    }

    /// True when the note of card `name` is "no sign of life", so the tile offers "Stop it".
    pub(super) fn can_stop(&self, name: &str) -> bool {
        matches!(
            self.ui.launch.notes.get(name),
            Some(LaunchReport::NoSignOfLife { .. })
        )
    }

    /// Opens the log of card `name` with the system's opener; a failure is a problem.
    pub fn open_log(&mut self, name: &str, launcher: &dyn Launcher) {
        self.problems.clear();
        let Some(path) = self.ui.launch.logs.get(name).cloned() else {
            return;
        };
        if let Err(detail) = launcher.open_url(&path.to_string_lossy()) {
            self.problems.push(Problem::LogOpenFailed { detail });
        }
    }

    /// True when card `name` has a log to open.
    pub(super) fn has_log(&self, name: &str) -> bool {
        self.ui.launch.logs.contains_key(name)
    }

    /// The note of card `name`: the first line says what happened, the others are the last lines of the log. Empty when nothing is to tell.
    pub(super) fn launch_note(&self, name: &str) -> Vec<String> {
        let lang = Lang::current();
        let title = self.title(name);
        match self.ui.launch.notes.get(name) {
            None | Some(LaunchReport::SignOfLife { .. }) => Vec::new(),
            Some(LaunchReport::EndedAtOnce { lines, .. }) => {
                let mut out = vec![words::fill(w::ended_at_once(lang), &[("name", &title)])];
                out.extend(lines.iter().cloned());
                out
            }
            Some(LaunchReport::NoSignOfLife { seconds, .. }) => vec![words::fill(
                w::no_sign_of_life(lang),
                &[("name", &title), ("seconds", &seconds.to_string())],
            )],
            Some(LaunchReport::Failed { code, lines, .. }) => {
                let head = match code {
                    Some(code) => words::fill(
                        w::ended_with_code(lang),
                        &[("name", &title), ("code", &code.to_string())],
                    ),
                    None => words::fill(w::ended_by_signal(lang), &[("name", &title)]),
                };
                let mut out = vec![head];
                if lines.is_empty() {
                    out.push(w::no_output(lang).to_owned());
                } else {
                    out.extend(lines.iter().cloned());
                }
                out
            }
            Some(LaunchReport::LogUnwritable { cause, .. }) => vec![words::fill(
                w::log_unwritable(lang),
                &[("name", &title), ("cause", cause)],
            )],
        }
    }
}
