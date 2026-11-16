//! The mark "Vorabversion" / "Pre-release" on a tile and on the details page: the demo store is drawn in German (`FS_LANG=de`) into
//! `FS_PACKAGES_SHOTS` when that is set, and the cards say so in the model. Where there is no graphics adapter the picture is skipped.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fs_components::blinc::core::reactive::global_graph;
use fs_components::blinc::core::{BlincContextState, HookState};
use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::theme::{ColorScheme, ThemeState};
use fs_components_render::{Canvas, RenderError, draw_to_png};
use fs_packages_app::demo::parts;
use fs_packages_app::model::StoreModel;
use fs_packages_app::view::store_app;

fn shoot(page: Option<&str>, file: &str) {
    // SAFETY: the only test of this file, nothing reads the environment before.
    unsafe { std::env::set_var("FS_LANG", "de") };
    let parts = parts(true);
    let mut model = StoreModel::new(parts.backend).with_updates(parts.updates);
    fs_components_render::use_style(None);
    model.refresh();
    let cards = model.cards();
    let flags: Vec<(&str, bool)> = cards
        .iter()
        .map(|c| (c.name.as_str(), c.prerelease))
        .collect();
    assert!(flags.contains(&("fs-people", true)), "{flags:?}");
    assert!(flags.contains(&("fs-tasks", true)), "{flags:?}");
    assert!(flags.contains(&("kanidm", false)), "{flags:?}");
    if let Some(name) = page {
        model.open_page(name);
    }
    let model = Arc::new(Mutex::new(model));
    let app = store_app(&model, &parts.launcher).unwrap();
    let bundle = fs_components_render::bundle(&app.styles());
    ThemeState::init(bundle, ColorScheme::Light);
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
    let Some(dir) = std::env::var_os("FS_PACKAGES_SHOTS").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let canvas = Canvas::new(1100, 760, app.styles());
    let compose = |w: f32, h: f32| -> Div { div().w(w).h(h).child(app.build(w, h)) };
    match draw_to_png(&canvas, compose, || {}, &dir.join(file)) {
        Err(RenderError::NoGraphics(why)) => eprintln!("skipped: {why}"),
        other => other.unwrap(),
    }
}

#[test]
fn tiles_and_page_show_the_prerelease_mark() {
    shoot(None, "kacheln-de.png");
    shoot(Some("fs-people"), "detailseite-de.png");
}
