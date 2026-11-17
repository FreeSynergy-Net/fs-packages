//! The note on the tile of a program that failed right after the start, and the "Open log" button: the model takes the launcher's reports
//! and says them on the card (the words come from the words module; the test checks what is language-independent).

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
        assert!(w::ended_with_code(lang).contains("{code}"));
        assert!(w::log_unwritable(lang).contains("{cause}"));
        assert!(!w::open_log(lang).is_empty());
    }
    assert_eq!(w::open_log(Lang::De), "Protokoll \u{f6}ffnen");
}
