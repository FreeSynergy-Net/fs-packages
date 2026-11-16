//! The three-dot menu of a tile stays inside the window: at the bottom edge it opens above its trigger and every line can be
//! pressed (the old Blinc menu opened below and was cut off). The window is laid out and driven without opening one (Blinc's
//! tree, router and overlay stack, in the order of its event loop). A picture is drawn too where there is a graphics adapter;
//! it stays in `FS_PACKAGES_SHOTS` if that is set (scene `more-unten`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_components::blinc::core::owner::BuildScope;
use fs_components::blinc::core::reactive::global_graph;
use fs_components::blinc::core::{BlincContextState, HookState};
use fs_components::blinc::layout::css_parser::Stylesheet;
use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::layout::event_router::{EventRouter, MouseButton};
use fs_components::blinc::layout::overlay_state::{
    overlay_stack, rebuild_overlay_subtree_if_dirty,
};
use fs_components::blinc::layout::prelude::RenderTree;
use fs_components::blinc::layout::tree::LayoutNodeId;
use fs_components::blinc::layout::widgets::overlay_stack::{
    OVERLAY_STACK_LAYER_ID, drain_close_callbacks,
};
use fs_components::blinc::theme::{ColorScheme, ThemeState};
use fs_components_render::{Canvas, RenderError, draw_to_png};
use fs_packages_app::demo::parts;
use fs_packages_app::model::StoreModel;
use fs_packages_app::view::store_app;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Left, top, width, height.
type Bounds = (f32, f32, f32, f32);

struct Window {
    app: fs_components_app::AppWidget,
    tree: RenderTree,
    router: EventRouter,
    size: (f32, f32),
    clock: u64,
    _serial: MutexGuard<'static, ()>,
}

/// What the window composes: the program's element and the overlay stack above it.
fn composed(app: &fs_components_app::AppWidget, w: f32, h: f32) -> Div {
    let layer = {
        let stack = overlay_stack();
        let mut stack = stack.lock().unwrap();
        stack.set_viewport_with_scale(w, h, 1.0);
        stack.build_overlay_layer()
    };
    div()
        .w(w)
        .h(h)
        .relative()
        .child(app.build(w, h))
        .child(layer)
}

/// No menu stays open (its state would outlive the test): close it and let the close report, as a frame does.
fn close_menus() {
    overlay_stack().lock().unwrap().close_all();
    drain_close_callbacks();
}

/// The demo store, `w` x `h`, the cards page.
fn window(w: f32, h: f32) -> Window {
    let serial = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let parts = parts(true);
    let mut model = StoreModel::new(parts.backend).with_updates(parts.updates);
    fs_components_render::use_style(None);
    model.refresh();
    let model = Arc::new(Mutex::new(model));
    let app = store_app(&model, &parts.launcher).unwrap();
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
    close_menus();
    let mut scope = BuildScope::new();
    let pass = scope.begin();
    fs_components::blinc::layout::key::reset_call_counters();
    let mut tree = RenderTree::from_element(&composed(&app, w, h));
    let vars = ThemeState::get().to_css_variable_map();
    let css = bundle.css_sources.join("\n");
    tree.set_stylesheet(Stylesheet::parse_with_variables(&css, &vars).unwrap());
    tree.apply_stylesheet_layout_overrides();
    tree.apply_stylesheet_base_styles();
    tree.compute_layout(w, h);
    scope.finish(pass);
    Window {
        app,
        tree,
        router: EventRouter::new(),
        size: (w, h),
        clock: 1,
        _serial: serial,
    }
}

impl Window {
    fn send(&mut self, events: Vec<(LayoutNodeId, u32)>, x: f32, y: f32) {
        for (node, event) in events {
            // each element gets its own bounds, as in the window: that is where a panel opens
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

    fn click(&mut self, (x, y): (f32, f32)) {
        let moved = self.router.on_mouse_move(&self.tree, x, y);
        self.send(moved, x, y);
        let down = self
            .router
            .on_mouse_down(&self.tree, x, y, MouseButton::Left);
        self.send(down, x, y);
        let up = self.router.on_mouse_up(&self.tree, x, y, MouseButton::Left);
        self.send(up, x, y);
        self.frame();
    }

    /// The window runs a frame: the overlay layer draws again and the layout follows.
    fn frame(&mut self) {
        self.clock += 16;
        drain_close_callbacks();
        overlay_stack().lock().unwrap().update(self.clock);
        let registry = Arc::clone(self.tree.element_registry());
        let dirty = overlay_stack().lock().unwrap().take_dirty();
        rebuild_overlay_subtree_if_dirty(&registry, OVERLAY_STACK_LAYER_ID, dirty, || {
            overlay_stack().lock().unwrap().build_overlay_layer()
        });
        let updates = self.tree.take_pending_property_updates();
        self.tree.apply_partial_property_updates(updates);
        self.tree
            .process_pending_subtree_rebuilds_routed(Some(&self.router));
        self.tree.apply_stylesheet_base_styles();
        self.tree.apply_stylesheet_layout_overrides();
        self.tree.compute_layout(self.size.0, self.size.1);
    }

    /// Nodes whose id starts with `prefix`, with their bounds.
    fn find(&self, prefix: &str) -> Vec<Bounds> {
        let registry = self.tree.element_registry();
        self.tree
            .iter_nodes()
            .filter_map(|(node, _)| {
                registry.get_id(node)?.starts_with(prefix).then_some(())?;
                let b = self.tree.get_absolute_bounds(node)?;
                Some((b.x, b.y, b.width, b.height))
            })
            .collect()
    }

    /// The triggers that lie wholly in the window, the lowest first.
    fn triggers_from_below(&self) -> Vec<Bounds> {
        let mut all: Vec<Bounds> = self
            .find("fs-dropdown-trigger-")
            .into_iter()
            .filter(|t| t.1 >= 0.0 && t.1 + t.3 <= self.size.1)
            .collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        all
    }
}

fn centre((x, y, w, h): Bounds) -> (f32, f32) {
    (x + w / 2.0, y + h / 2.0)
}

/// The usual window of the store (900x640): the lowest tile in view has about 60 px below its trigger, the menu is 107 px high.
const SHORT: (f32, f32) = (900.0, 640.0);

#[test]
fn the_menu_of_the_lowest_tile_opens_above_its_trigger_inside_the_window() {
    let mut w = window(SHORT.0, SHORT.1);
    let triggers = w.triggers_from_below();
    assert!(!triggers.is_empty(), "no three-dot trigger in the window");
    let trigger = triggers[0];
    w.click(centre(trigger));
    let panels = w.find("fs-dropdown-panel-");
    assert_eq!(panels.len(), 1, "the menu did not open");
    let (x, y, pw, ph) = panels[0];
    assert!(
        x >= 0.0 && y >= 0.0 && x + pw <= SHORT.0 && y + ph <= SHORT.1,
        "panel {panels:?} not inside the window {SHORT:?} (trigger {trigger:?})"
    );
    // directly above the trigger (the gap is 4 px), not somewhere else in the window
    let above = y + ph <= trigger.1 && trigger.1 - (y + ph) < 12.0;
    close_menus();
    let room_below = SHORT.1 - (trigger.1 + trigger.3);
    assert!(
        room_below < ph,
        "the test needs a trigger with too little room below ({room_below} px for {ph})"
    );
    assert!(above, "panel {panels:?} is not above {trigger:?}");
}

#[test]
fn the_open_menu_is_drawn_inside_the_window() {
    let mut w = window(SHORT.0, SHORT.1);
    let trigger = w.triggers_from_below()[0];
    w.click(centre(trigger));
    let dir = std::env::var_os("FS_PACKAGES_SHOTS").map_or_else(
        || std::env::temp_dir().join(format!("fs-packages-more-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        w.find("fs-dropdown-panel-").len(),
        1,
        "the menu is not open before the picture"
    );
    let path = dir.join("more-unten.png");
    let canvas = Canvas::new(900, 640, w.app.styles());
    match draw_to_png(&canvas, |a, b| composed(&w.app, a, b), || {}, &path) {
        Err(RenderError::NoGraphics(why)) => eprintln!("skipped: {why}"),
        other => other.unwrap(),
    }
    if std::env::var_os("FS_PACKAGES_SHOTS").is_none() {
        let _ = std::fs::remove_file(&path);
    }
    close_menus();
}
