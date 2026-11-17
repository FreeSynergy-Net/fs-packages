//! The cards are computed once for each draw, not once for each place that wants them; and never reused between draws.

mod common;

use std::sync::{Arc, Mutex};

use fs_packages_app::launcher::Launcher;
use fs_packages_app::model::Section;
use fs_packages_app::view::store_app;

use common::{fake, model};

#[test]
fn inside_a_draw_the_cards_are_computed_once_outside_every_time() {
    let f = fake(Vec::new(), None);
    let mut m = model(&f);
    let base = m.cards_computed();
    let _ = (m.cards(), m.cards());
    assert_eq!(
        m.cards_computed() - base,
        2,
        "outside a draw: fresh each time"
    );

    let base = m.cards_computed();
    {
        let draw = m.frame_cards();
        let _ = (m.cards(), m.cards_shared(), m.listing(), m.tab_counts());
        let _ = (m.nav(), m.start_blocks(), m.selected());
        assert_eq!(m.cards_computed() - base, 1, "inside a draw: once");
        drop(draw);
    }
    // the draw is over: the next call sees the model as it is now
    assert!(m.hover("fs-theme"));
    assert!(m.cards().iter().any(|c| c.name == "fs-theme" && c.hovered));
    assert_eq!(m.cards_computed() - base, 2);
    m.open_section(Section::Start);
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
