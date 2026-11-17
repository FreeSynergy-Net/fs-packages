//! The note on the tile of a program that failed right after the start, ended at once, or shows no sign of life, and the "Open log" and
//! "Stop it" buttons: the model takes the launcher's reports and says them on the card (the words come from the words module; the test checks what is language-independent).

#![allow(clippy::assert_is_empty)]

mod common;

use std::path::PathBuf;
use std::sync::Mutex;

use fs_packages_app::backend::InstalledRow;
use fs_packages_app::launch_watch::LaunchReport;
use fs_packages_app::launcher::Launcher;

use common::{fake, model, v};

struct Reporting {
    reports: Mutex<Vec<LaunchReport>>,
    opened: Mutex<Vec<String>>,
    stopped: Mutex<Vec<String>>,
    log: Option<PathBuf>,
}

impl Launcher for Reporting {
    fn exists(&self, command: &str) -> bool {
        command == "fs-tasks"
    }
    fn start(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.to_owned());
        Ok(())
    }
    fn take_reports(&self) -> Vec<LaunchReport> {
        std::mem::take(&mut *self.reports.lock().unwrap())
    }
    fn log_path(&self, _: &str) -> Option<PathBuf> {
        self.log.clone()
    }
    fn stop(&self, command: &str) -> bool {
        self.stopped.lock().unwrap().push(command.to_owned());
        true
    }
}

fn setup(log: bool) -> (fs_packages_app::model::StoreModel, Reporting) {
    let f = fake(
        vec![InstalledRow {
            name: "tasks".into(),
            version: v(0, 1, 0),
        }],
        None,
    );
    let launcher = Reporting {
        reports: Mutex::new(Vec::new()),
        opened: Mutex::new(Vec::new()),
        stopped: Mutex::new(Vec::new()),
        log: log.then(|| PathBuf::from("/state/fs-packages/launch-fs-tasks.log")),
    };
    (model(&f), launcher)
}

fn card(m: &fs_packages_app::model::StoreModel) -> fs_packages_app::model::Card {
    m.cards().into_iter().find(|c| c.name == "tasks").unwrap()
}

#[test]
fn an_early_failure_is_on_the_tile_with_the_last_lines() {
    let (mut m, l) = setup(true);
    m.open("tasks", None, &l);
    assert!(card(&m).launch_note.is_empty());
    l.reports.lock().unwrap().push(LaunchReport::Failed {
        command: "fs-tasks".into(),
        code: Some(3),
        lines: vec!["socket folder is open (0755)".into(), "ending".into()],
    });
    assert!(m.sync_launch(&l));
    let note = card(&m).launch_note;
    assert_eq!(note.len(), 3);
    assert!(note[0].contains('3'), "{}", note[0]);
    assert_eq!(note[1..], ["socket folder is open (0755)", "ending"]);
    // A new start takes the old note away.
    m.open("tasks", None, &l);
    assert!(card(&m).launch_note.is_empty());
}

#[test]
fn a_log_that_could_not_be_written_is_told_with_the_cause() {
    let (mut m, l) = setup(false);
    m.open("tasks", None, &l);
    l.reports.lock().unwrap().push(LaunchReport::LogUnwritable {
        command: "fs-tasks".into(),
        cause: "Permission denied".into(),
    });
    m.sync_launch(&l);
    let note = card(&m).launch_note;
    assert_eq!(note.len(), 1);
    assert!(note[0].contains("Permission denied"));
    assert!(!card(&m).log_available);
}

#[test]
fn open_log_shows_only_when_a_log_exists_and_opens_that_file() {
    let (mut m, none) = setup(false);
    m.sync_launch(&none);
    assert!(!card(&m).log_available);
    let (mut m, l) = setup(true);
    assert!(
        !card(&m).log_available,
        "nothing is known before the first look"
    );
    m.sync_launch(&l);
    assert!(
        card(&m).log_available,
        "the log of an earlier session counts"
    );
    m.open_log("tasks", &l);
    assert_eq!(
        *l.opened.lock().unwrap(),
        ["/state/fs-packages/launch-fs-tasks.log"]
    );
    assert!(m.problems().is_empty());
}

#[test]
fn the_words_exist_in_both_languages() {
    use fs_packages_app::words_front::Lang;
    use fs_packages_app::words_launch as w;
    for lang in [Lang::En, Lang::De] {
        assert!(w::ended_at_once(lang).contains("{name}"));
        assert!(w::no_sign_of_life(lang).contains("{seconds}"));
        assert!(!w::stop_program(lang).is_empty());
        assert!(w::ended_with_code(lang).contains("{code}"));
        assert!(w::log_unwritable(lang).contains("{cause}"));
        assert!(!w::open_log(lang).is_empty());
    }
    assert_eq!(w::open_log(Lang::De), "Protokoll \u{f6}ffnen");
}

#[test]
fn an_early_clean_end_is_a_hint_with_the_lines() {
    let (mut m, l) = setup(true);
    m.open("tasks", None, &l);
    l.reports.lock().unwrap().push(LaunchReport::EndedAtOnce {
        command: "fs-tasks".into(),
        lines: vec!["handed over".into()],
    });
    assert!(m.sync_launch(&l));
    let note = card(&m).launch_note;
    assert_eq!(note.len(), 2);
    assert!(note[0].contains('0'), "{}", note[0]);
    assert_eq!(note[1], "handed over");
    assert!(!card(&m).can_stop, "nothing to stop: it ended");
}

#[test]
fn no_sign_of_life_is_on_the_tile_with_stop_and_goes_when_the_sign_comes_or_it_is_stopped() {
    let (mut m, l) = setup(true);
    m.open("tasks", None, &l);
    assert!(!card(&m).can_stop);
    l.reports.lock().unwrap().push(LaunchReport::NoSignOfLife {
        command: "fs-tasks".into(),
        seconds: 8,
    });
    m.sync_launch(&l);
    let c = card(&m);
    assert!(c.can_stop);
    assert_eq!(c.launch_note.len(), 1);
    assert!(c.launch_note[0].contains('8'), "{}", c.launch_note[0]);
    // A sign of life takes the hint back.
    l.reports.lock().unwrap().push(LaunchReport::SignOfLife {
        command: "fs-tasks".into(),
    });
    m.sync_launch(&l);
    assert!(card(&m).launch_note.is_empty());
    assert!(!card(&m).can_stop);
    // Stopping goes through the launcher, by the command the card was started with, and clears the note.
    l.reports.lock().unwrap().push(LaunchReport::NoSignOfLife {
        command: "fs-tasks".into(),
        seconds: 9,
    });
    m.sync_launch(&l);
    assert!(m.stop_program("tasks", &l));
    assert_eq!(*l.stopped.lock().unwrap(), ["fs-tasks"]);
    assert!(card(&m).launch_note.is_empty());
    assert!(
        !m.stop_program("unknown", &l),
        "a card that was never started"
    );
    assert_eq!(l.stopped.lock().unwrap().len(), 1);
}

#[test]
fn every_report_has_its_own_code() {
    let reports = [
        LaunchReport::Failed {
            command: "x".into(),
            code: None,
            lines: Vec::new(),
        },
        LaunchReport::EndedAtOnce {
            command: "x".into(),
            lines: Vec::new(),
        },
        LaunchReport::NoSignOfLife {
            command: "x".into(),
            seconds: 1,
        },
        LaunchReport::SignOfLife {
            command: "x".into(),
        },
        LaunchReport::LogUnwritable {
            command: "x".into(),
            cause: String::new(),
        },
    ];
    let mut codes: Vec<&str> = reports.iter().map(LaunchReport::code).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), 5);
    assert!(reports.iter().all(|r| r.command() == "x"));
}
