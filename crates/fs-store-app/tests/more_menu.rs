//! The three-dot menu of a tile opens on a click, for every card: not installed and installed by rpm (the backend lists the
//! package; nothing in `installed.toml`, no `current/`), at several window sizes, on the start page and in a section, and after the
//! list was scrolled (then the menu must open at the tile). The window is laid out and driven without opening one.

mod common;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use common::{fake, package, v};
use fs_components::blinc::core::owner::BuildScope;
use fs_components::blinc::core::reactive::global_graph;
use fs_components::blinc::core::{BlincContextState, HookState};
use fs_components::blinc::layout::css_parser::Stylesheet;
use fs_components::blinc::layout::event_router::{EventRouter, MouseButton};
use fs_components::blinc::layout::overlay_state::overlay_stack;
use fs_components::blinc::layout::prelude::RenderTree;
use fs_components::blinc::layout::tree::LayoutNodeId;
use fs_components::blinc::theme::{ColorScheme, ThemeState};
use fs_store_app::backend::InstalledRow;
use fs_store_app::launcher::Launcher;
use fs_store_app::model::{Section, StoreModel};
use fs_store_app::view::store_app;
use fs_store_core::Kind;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct NoLauncher;
impl Launcher for NoLauncher {
    fn exists(&self, _name: &str) -> bool {
        false
    }
    fn start(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, _url: &str) -> Result<(), String> {
        Ok(())
    }
}

struct Window {
    tree: RenderTree,
    router: EventRouter,
    _serial: MutexGuard<'static, ()>,
}

fn window(installed: &[(&str, u64)], size: (f32, f32), section: bool) -> Window {
    let serial = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let rows = installed
        .iter()
        .map(|(n, patch)| InstalledRow {
            name: (*n).to_owned(),
            version: v(0, 0, *patch),
        })
        .collect();
    let f = fake(rows, None);
    f.state
        .lock()
        .unwrap()
        .offered
        .push(package("fs-wiki", v(0, 0, 12)));
    let mut model = StoreModel::new(f);
    fs_components_render::use_style(Some(fs_components_styles::StyleId::Craft));
    model.refresh();
    if section {
        model.open_section(Section::Kind(Kind::Program));
    }
    let model = Arc::new(Mutex::new(model));
    let launcher: Arc<dyn Launcher> = Arc::new(NoLauncher);
    let app = store_app(&model, &launcher).unwrap();
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
    let mut scope = BuildScope::new();
    let pass = scope.begin();
    fs_components::blinc::layout::key::reset_call_counters();
    let mut tree = RenderTree::from_element(&app.build(size.0, size.1));
    let vars = ThemeState::get().to_css_variable_map();
    let css = bundle.css_sources.join("\n");
    tree.set_stylesheet(Stylesheet::parse_with_variables(&css, &vars).unwrap());
    tree.apply_stylesheet_layout_overrides();
    tree.apply_stylesheet_base_styles();
    tree.compute_layout(size.0, size.1);
    scope.finish(pass);
    Window {
        tree,
        router: EventRouter::new(),
        _serial: serial,
    }
}

impl Window {
    fn click_node(&mut self, node: LayoutNodeId) {
        let b = self.tree.get_absolute_bounds(node).expect("bounds");
        let (x, y) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
        let moved = self.router.on_mouse_move(&self.tree, x, y);
        self.send(moved, x, y);
        let down = self
            .router
            .on_mouse_down(&self.tree, x, y, MouseButton::Left);
        self.send(down, x, y);
        let up = self.router.on_mouse_up(&self.tree, x, y, MouseButton::Left);
        self.send(up, x, y);
    }

    fn send(&mut self, events: Vec<(LayoutNodeId, u32)>, x: f32, y: f32) {
        // as the window's event loop does: with the bounds of the node the handler sits on
        for (node, event) in events {
            let (bx, by, bw, bh) = self.router.get_node_bounds(node).unwrap_or_default();
            self.tree.dispatch_event_full(
                node,
                event,
                x,
                y,
                x - bx,
                y - by,
                bx,
                by,
                bw,
                bh,
                0.0,
                0.0,
                1.0,
                false,
                false,
                false,
                false,
            );
        }
    }

    /// The triggers of the three-dot menus, in the order the tiles are built.
    fn triggers(&self) -> Vec<LayoutNodeId> {
        let registry = self.tree.element_registry();
        let mut found: Vec<(usize, LayoutNodeId)> = self
            .tree
            .iter_nodes()
            .filter_map(|(node, _)| {
                let id = registry.get_id(node)?;
                let at = id.strip_prefix("cn-dropdown-trigger-dropdown:")?;
                Some((at.rsplit(':').next()?.parse().ok()?, node))
            })
            .collect();
        found.sort_unstable();
        found.into_iter().map(|(_, node)| node).collect()
    }
}

fn open_overlays() -> usize {
    overlay_stack().lock().unwrap().len()
}

/// Forget the open state of every menu, as the window's frame loop does when an overlay has finished closing.
fn reset_menus(count: usize) {
    overlay_stack().lock().unwrap().close_all();
    for index in 0..count {
        let key = format!("dropdown:crates/fs-store-app/src/view/more.rs:44:16:{index}");
        let ctx = BlincContextState::get();
        ctx.use_signal_keyed(&key, || false).set(false);
        ctx.use_signal_keyed(&format!("{key}_handle"), || None::<u64>)
            .set(None);
    }
}

/// Click the trigger of card `index` and say whether a menu came up.
fn opens(w: &mut Window, index: usize) -> bool {
    reset_menus(8);
    let before = open_overlays();
    let node = w.triggers()[index];
    w.click_node(node);
    let up = open_overlays() > before;
    reset_menus(8);
    up
}

const SIZES: [(f32, f32); 4] = [
    (1100.0, 900.0),
    (900.0, 640.0),
    (1400.0, 900.0),
    (600.0, 700.0),
];

fn every_tile_opens() {
    for installed in [&[][..], &[("fs-wiki", 12)][..]] {
        for size in SIZES {
            for section in [false, true] {
                let mut w = window(installed, size, section);
                let count = w.triggers().len();
                assert!(count >= 4 || !section, "{count} menus at {size:?}");
                for index in 0..count {
                    let b = w.tree.get_absolute_bounds(w.triggers()[index]).unwrap();
                    if b.y + b.height > size.1 || b.x + b.width > size.0 {
                        continue; // scrolled out of the window
                    }
                    assert!(
                        opens(&mut w, index),
                        "menu {index} of {count} did not open ({installed:?}, {size:?}, section {section})"
                    );
                }
            }
        }
    }
}

fn a_scrolled_tile_opens_where_it_is() {
    let mut w = window(&[("fs-wiki", 12)], (700.0, 260.0), true);
    let (x, y) = (350.0, 150.0);
    let moved = w.router.on_mouse_move(&w.tree, x, y);
    w.send(moved, x, y);
    let hit = w
        .router
        .on_scroll_nested(&w.tree, 0.0, -2000.0)
        .expect("hit");
    w.tree
        .dispatch_scroll_chain(hit.node, &hit.ancestors, x, y, 0.0, -2000.0);
    let count = w.triggers().len();
    let mut checked = 0;
    for index in 0..count {
        let node = w.triggers()[index];
        let b = w.tree.get_absolute_bounds(node).unwrap();
        if b.y < 0.0 || b.y + b.height > 260.0 {
            continue;
        }
        reset_menus(8);
        let before = open_overlays();
        w.click_node(node);
        assert!(
            open_overlays() > before,
            "menu {index} did not open at {b:?}"
        );
        let stack = overlay_stack();
        let stack = stack.lock().unwrap();
        let entry = stack.top().unwrap();
        let shown = format!("{:?}", entry.position);
        let top: f32 = shown
            .split("y: ")
            .nth(1)
            .and_then(|t| t.trim_end_matches([' ', '}']).parse().ok())
            .unwrap_or_else(|| panic!("no y in {shown}"));
        assert!(
            (top - (b.y + b.height)).abs() < 40.0,
            "menu {index} opens at y={top}, the trigger is at {b:?}"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "nothing visible after the scroll ({count} menus)"
    );
}

/// One test, not two: the overlay stack and the click-outside registry of Blinc are process-wide, and a second window in the same
/// process starts with what the first left in them.
#[test]
fn the_three_dot_menu_opens_for_every_tile_also_when_installed_by_rpm_and_when_scrolled() {
    every_tile_opens();
    a_scrolled_tile_opens_where_it_is();
}
