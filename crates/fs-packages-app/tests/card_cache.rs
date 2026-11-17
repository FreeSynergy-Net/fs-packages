//! The cards are kept between draws: computed again only when the model or the language changed, never because of the width.

mod common;

use std::sync::{Arc, Mutex};

use fs_packages_app::launcher::Launcher;
use fs_packages_app::view::store_app;

use common::{fake, model, package, v};

#[test]
fn the_same_model_gives_the_same_cards_without_computing_again() {
    let f = fake(Vec::new(), None);
    let m = model(&f);
    let base = m.cards_computed();
    let first = m.cards();
    let second = m.cards();
    let shared = m.cards_shared();
    assert_eq!(m.cards_computed() - base, 1, "same input: computed once");
    assert_eq!(first, second);
    assert_eq!(first, *shared);
    // the other readers of the cards reuse them too
    let _ = (
        m.listing(),
        m.tab_counts(),
        m.nav(),
        m.start_blocks(),
        m.selected(),
    );
    assert_eq!(m.cards_computed() - base, 1);
}

#[test]
fn a_change_of_the_model_computes_the_cards_again() {
    let f = fake(Vec::new(), None);
    let mut m = model(&f);
    let base = m.cards_computed();
    assert!(m.cards().iter().all(|c| !c.hovered));
    assert_eq!(m.cards_computed() - base, 1);
    // hover: a card field
    assert!(m.hover("fs-theme"));
    assert!(m.cards().iter().any(|c| c.name == "fs-theme" && c.hovered));
    assert_eq!(m.cards_computed() - base, 2);
    // selection
    m.select("fs-tasks");
    assert!(m.cards().iter().any(|c| c.name == "fs-tasks" && c.selected));
    assert_eq!(m.cards_computed() - base, 3);
    // catalog: a program more on offer
    f.state
        .lock()
        .unwrap()
        .offered
        .push(package("fs-new", v(0, 0, 1)));
    m.refresh();
    assert!(m.cards().iter().any(|c| c.name == "fs-new"));
    assert_eq!(m.cards_computed() - base, 4);
    // installed programs
    m.run_job("fs-new");
    assert!(
        m.cards()
            .iter()
            .any(|c| c.name == "fs-new" && c.installed.is_some())
    );
    assert!(
        m.cards_computed() - base >= 5,
        "jobs and installed rows invalidate"
    );
    // nothing changed since: nothing computed
    let now = m.cards_computed();
    let _ = (m.cards(), m.cards());
    assert_eq!(m.cards_computed(), now);
}

struct NoLauncher;
impl Launcher for NoLauncher {
    fn exists(&self, _: &str) -> bool {
        false
    }
    fn start(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "no GPU or software adapter on the Windows runner (Linux CI uses lavapipe): drawing is not checked here yet"
)]
fn a_resize_does_not_compute_the_cards_again() {
    let f = fake(Vec::new(), None);
    let shared = Arc::new(Mutex::new(model(&f)));
    let launcher: Arc<dyn Launcher> = Arc::new(NoLauncher);
    let app = store_app(&shared, &launcher).unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fs-packages-resize-{}.png", std::process::id()));
    app.picture(900, 640, &path).unwrap();
    let after_first = shared.lock().unwrap().cards_computed();
    assert_eq!(after_first, 1, "the first draw computes them once");
    for (w, h) in [(901, 640), (1100, 700), (700, 500), (1600, 900)] {
        app.picture(w, h, &path).unwrap();
    }
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        shared.lock().unwrap().cards_computed(),
        after_first,
        "the width only lays out; the cards stay"
    );
    // the model changes: the next draw computes them again
    shared.lock().unwrap().hover("fs-theme");
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fs-packages-resize2-{}.png", std::process::id()));
    app.picture(900, 640, &path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(shared.lock().unwrap().cards_computed(), after_first + 1);
}

#[test]
fn the_resize_path_of_14_cards_is_fast() {
    // The path a resize frame takes for the cards: the fingerprint + the kept cards. The real number is 8 ms x 14 = 119 ms before
    // (a process per card); here 14 cards from the fixture, 200 reads, average under 5 ms per read even in a debug build on a
    // slow CI machine (release: ~0.05 ms). Debug-friendly threshold, because the point is "not a process per card".
    let f = fake(Vec::new(), None);
    f.state.lock().unwrap().offered = (0..14)
        .map(|i| package(&format!("fs-p{i:02}"), v(0, 1, i)))
        .collect();
    let m = model(&f);
    let _ = m.cards();
    let t = std::time::Instant::now();
    for _ in 0..200 {
        let _ = m.cards_shared();
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / 200.0;
    assert!(per < 5.0, "kept cards read in {per:.3} ms");
    assert_eq!(m.cards_computed(), 1);
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "no GPU or software adapter on the Windows runner (Linux CI uses lavapipe): drawing is not checked here yet"
)]
fn the_storefront_build_computes_the_cards_once() {
    let f = fake(Vec::new(), None);
    let shared = Arc::new(Mutex::new(model(&f)));
    let launcher: Arc<dyn Launcher> = Arc::new(NoLauncher);
    let app = store_app(&shared, &launcher).unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fs-packages-cache-{}.png", std::process::id()));
    let before = shared.lock().unwrap().cards_computed();
    app.picture(900, 640, &path).unwrap();
    let _ = std::fs::remove_file(&path);
    let after = shared.lock().unwrap().cards_computed();
    assert_eq!(after - before, 1, "one build, one computation of the cards");
}
