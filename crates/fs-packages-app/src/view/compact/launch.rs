//! Pattern: Adapter (what a started program has to tell, in the compact variant: the note, "Open log" and, for one that does not
//! respond, "Stop it").

use std::sync::Arc;

use fs_components::blinc::cn::components::button::ButtonVariant;
use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::layout::prelude::text;

use crate::launcher::Launcher;
use crate::model::Card;
use crate::view::shell::{Shared, button, locked, rebuild};
use crate::words_lang::Lang;
use crate::words_launch as w;

/// Adds the note of the started program (first line says what happened, the others are the end of its log) and its buttons to `pane`.
pub(super) fn launch_part(
    mut pane: Div,
    card: &Card,
    shared: &Shared,
    launcher: &Arc<dyn Launcher>,
) -> Div {
    for line in &card.launch_note {
        pane = pane.child(text(line.clone()).class("fs-c-sub"));
    }
    let mut row = div().flex_row().gap(2.0);
    if card.log_available {
        let (shared, launcher, name) =
            (Arc::clone(shared), Arc::clone(launcher), card.name.clone());
        row = row.child(
            button(
                &format!("c-log-{name}"),
                w::open_log(Lang::current()),
                ButtonVariant::Outline,
                true,
            )
            .on_click(move |_| {
                locked(&shared).open_log(&name, launcher.as_ref());
                rebuild();
            }),
        );
    }
    if card.can_stop {
        let (shared, launcher, name) =
            (Arc::clone(shared), Arc::clone(launcher), card.name.clone());
        row = row.child(
            button(
                &format!("c-stop-{name}"),
                w::stop_program(Lang::current()),
                ButtonVariant::Outline,
                true,
            )
            .on_click(move |_| {
                locked(&shared).stop_program(&name, launcher.as_ref());
                rebuild();
            }),
        );
    }
    if card.log_available || card.can_stop {
        pane = pane.child(row);
    }
    pane
}
