//! The cost of a resize frame on our side, without a window: build of the whole screen, tree, stylesheet and layout at changing widths
//! (what the event loop does for every resize frame, minus Blinc's painting) over 14 cards. The cards are computed once, not per frame.
//!
//! Release, with numbers: `cargo test -p fs-packages-app --release --test resize_cost -- --nocapture`.
//! The limit of the checked test is large (debug build, slow CI machines): it exists to catch "a process per card" or "cards per frame",
//! which cost 100 ms and more; the 16 ms target of a frame is measured in the real window (docs/measuring.md).

mod common;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use fs_components::blinc::core::owner::BuildScope;
use fs_components::blinc::core::reactive::global_graph;
use fs_components::blinc::core::{BlincContextState, HookState};
use fs_components::blinc::layout::css_parser::Stylesheet;
use fs_components::blinc::layout::prelude::RenderTree;
use fs_components::blinc::theme::{ColorScheme, ThemeState};
use fs_packages_app::launcher::Launcher;
use fs_packages_app::view::store_app;

use common::{fake, model, package, v};

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
fn a_resize_frame_over_14_cards_does_not_recompute_the_cards() {
    let f = fake(Vec::new(), None);
    f.state.lock().unwrap().offered = (0..14)
        .map(|i| package(&format!("fs-program-{i:02}"), v(0, 1, i)))
        .collect();
    let shared = Arc::new(Mutex::new(model(&f)));
    let launcher: Arc<dyn Launcher> = Arc::new(NoLauncher);
    let app = store_app(&shared, &launcher).unwrap();
    let bundle = fs_components_render::bundle(&app.styles());
    ThemeState::init(bundle.clone(), ColorScheme::Light);
    if !BlincContextState::is_initialized() {
        BlincContextState::init_with_callback(
            global_graph(),
            Arc::new(Mutex::new(HookState::new())),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Arc::new(|ids: &[_]| {
                fs_components::blinc::layout::check_stateful_deps(ids);
            }),
        );
    }
    fs_components::blinc::layout::init_text_measurer();
    let vars = ThemeState::get().to_css_variable_map();
    let css = bundle.css_sources.join("\n");
    let sheet = Stylesheet::parse_with_variables(&css, &vars).unwrap();
    // (build of our elements, tree from the elements, stylesheet applied, layout) in ms
    let frame = |w: f32, h: f32| -> [f64; 4] {
        let mut scope = BuildScope::new();
        let pass = scope.begin();
        fs_components::blinc::layout::key::reset_call_counters();
        let t = Instant::now();
        let element = app.build(w, h);
        let built = t.elapsed();
        let t = Instant::now();
        let mut tree = RenderTree::from_element(&element);
        let treed = t.elapsed();
        let t = Instant::now();
        tree.set_stylesheet(sheet.clone());
        tree.apply_stylesheet_layout_overrides();
        tree.apply_stylesheet_base_styles();
        let styled = t.elapsed();
        let t = Instant::now();
        tree.compute_layout(w, h);
        let laid = t.elapsed();
        scope.finish(pass);
        [built, treed, styled, laid].map(|d| d.as_secs_f64() * 1000.0)
    };
    frame(900.0, 640.0);
    let cards = shared.lock().unwrap().cards().len();
    assert_eq!(cards, 14);
    let base = shared.lock().unwrap().cards_computed();
    let n = 40u16;
    let mut sum = [0.0f64; 4];
    for i in 0..n {
        let one = frame(700.0 + f32::from(i) * 10.0, 640.0);
        for (s, o) in sum.iter_mut().zip(one) {
            *s += o;
        }
    }
    let [built, treed, styled, laid] = sum.map(|s| s / f64::from(n));
    let per = built + treed + styled + laid;
    println!(
        "{cards} cards: {per:.2} ms per resize frame, no painting (our build {built:.2}, tree {treed:.2}, stylesheet {styled:.2}, layout {laid:.2})"
    );
    assert_eq!(
        shared.lock().unwrap().cards_computed(),
        base,
        "the cards stay while only the width changes"
    );
    assert!(per < 60.0, "a resize frame took {per:.1} ms on our side");
}
