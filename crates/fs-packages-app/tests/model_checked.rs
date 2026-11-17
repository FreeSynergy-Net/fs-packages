//! The footer line "Checked ..." follows the catalog reads: a good read moves it to "just now", a failed one leaves it and tells the
//! failure, a listing from memory keeps the time of its own fetch (the time is the backend's, so no real clock is needed).

mod common;

use std::sync::{Arc, Mutex};

use fs_packages_app::backend::InstalledRow;
use fs_packages_app::model::StoreModel;
use fs_packages_app::updates::UpdateService;
use fs_packages_core::Version;
use fs_packages_sources::{Findings, Settings};

use common::fake;

struct Clock(Mutex<u64>);

impl UpdateService for Clock {
    fn settings(&self) -> Result<Settings, String> {
        Ok(Settings::default())
    }
    fn save_settings(&self, _: &Settings) -> Result<(), String> {
        Ok(())
    }
    fn findings(&self) -> Result<Findings, String> {
        Ok(Findings {
            checked: 1_000,
            ..Findings::default()
        })
    }
    fn save_findings(&self, _: &Findings) -> Result<(), String> {
        Ok(())
    }
    fn check(&self) -> Result<(), String> {
        Ok(())
    }
    fn now(&self) -> u64 {
        *self.0.lock().unwrap()
    }
}

fn english() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe { std::env::set_var("FS_LANG", "en") });
}

#[test]
fn a_good_read_sets_checked_to_just_now_and_a_failed_one_keeps_the_old_time() {
    english();
    let backend = fake(Vec::new(), None);
    let clock = Arc::new(Clock(Mutex::new(1_000 + 16 * 60)));
    let mut m = StoreModel::new(backend.clone()).with_updates(clock.clone());
    assert_eq!(m.footer_checked(), "Checked 16 min ago");
    // a fresh fetch at "now"
    *backend.read_at.lock().unwrap() = 1_000 + 16 * 60;
    m.refresh();
    assert_eq!(m.footer_checked(), "Checked just now");
    // time passes; a listing from memory keeps the time of its fetch
    *clock.0.lock().unwrap() += 5 * 60;
    m.refresh();
    assert_eq!(m.footer_checked(), "Checked 5 min ago");
    // a read that fails leaves the old time and tells the failure
    *backend.fail_available.lock().unwrap() = true;
    *backend.read_at.lock().unwrap() = 1_000 + 21 * 60;
    m.refresh();
    assert_eq!(m.footer_checked(), "Checked 5 min ago");
    assert!(!m.problems().is_empty(), "the failure is told");
}

#[test]
fn an_installed_program_that_no_source_offers_says_so_and_a_down_source_does_not() {
    english();
    let backend = fake(
        vec![InstalledRow {
            name: "fs-gone".into(),
            version: Version::new(0, 1, 0),
        }],
        None,
    );
    let mut m = StoreModel::new(backend.clone());
    m.refresh();
    let gone = m.cards().into_iter().find(|c| c.name == "fs-gone").unwrap();
    assert!(gone.no_longer_offered);
    assert_eq!(gone.meta_line(), "No longer offered");
    assert!(gone.description_text().starts_with("No longer offered"));
    // a program that is offered is not marked
    let offered = m
        .cards()
        .into_iter()
        .find(|c| c.name == "fs-tasks")
        .unwrap();
    assert!(!offered.no_longer_offered);
}
