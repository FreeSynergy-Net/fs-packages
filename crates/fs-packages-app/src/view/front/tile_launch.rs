//! Pattern: Adapter (what a started program has to tell, on its tile: the note, "Open log" and, for one that does not respond, "Stop it").

use std::sync::Arc;

use fs_components::blinc::layout::div::Div;
use fs_components::blinc::layout::prelude::text;
use fs_components_widgets::craft::link;

use crate::launcher::Launcher;
use crate::model::Card;
use crate::view::shell::{Shared, locked, rebuild};
use crate::words_lang::Lang;
use crate::words_launch as w;

/// "Open log": opens the log of the started program with the system's opener.
fn log_link(card: &Card, shared: &Shared, launcher: &Arc<dyn Launcher>) -> Div {
    let (shared, launcher, name) = (Arc::clone(shared), Arc::clone(launcher), card.name.clone());
    link(
        &format!("log-{}", card.name),
        w::open_log(Lang::current()),
        Arc::new(move || {
            locked(&shared).open_log(&name, launcher.as_ref());
            rebuild();
        }),
    )
}

/// "Stop it": ends the process the store started for this program (only that one).
fn stop_link(card: &Card, shared: &Shared, launcher: &Arc<dyn Launcher>) -> Div {
    let (shared, launcher, name) = (Arc::clone(shared), Arc::clone(launcher), card.name.clone());
    link(
        &format!("stop-{}", card.name),
        w::stop_program(Lang::current()),
        Arc::new(move || {
            locked(&shared).stop_program(&name, launcher.as_ref());
            rebuild();
        }),
    )
}

/// Adds the note of the started program (first line says what happened, the others are the end of its log) and its links to `body`.
pub(super) fn launch_part(
    mut body: Div,
    card: &Card,
    shared: &Shared,
    launcher: &Arc<dyn Launcher>,
) -> Div {
    for (i, line) in card.launch_note.iter().enumerate() {
        let class = if i == 0 {
            "craft-sub"
        } else {
            "craft-caps-dim"
        };
        body = body.child(text(line.clone()).class(class));
    }
    if card.log_available {
        body = body.child(log_link(card, shared, launcher));
    }
    if card.can_stop {
        body = body.child(stop_link(card, shared, launcher));
    }
    body
}
