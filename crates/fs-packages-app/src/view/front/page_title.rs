//! Pattern: Adapter (the heading block of the package page: the name, the kind and version line, and the pre-release mark when the
//! offered release is one). Kept apart so a test can count what the heading holds.

use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::layout::prelude::text;

use crate::model::Card;
use crate::view::card::prerelease_badge;

/// The heading of the page: `title`, the line `kind_version` (already in capitals) and, for a pre-release, the badge.
pub(super) fn page_title(card: &Card, kind_version: String) -> Div {
    div()
        .flex_col()
        .flex_grow()
        .min_w(0.0)
        .gap(1.0)
        .child(text(card.title.clone()).size(24.0).bold())
        .child(text(kind_version).class("craft-caps-dim"))
        .children(prerelease_badge(card))
}

#[cfg(test)]
mod tests {
    use fs_components::blinc::layout::ElementBuilder;

    use super::page_title;
    use crate::view::card::tests::demo_card as card;

    #[test]
    fn the_heading_holds_the_badge_for_a_prerelease_and_not_for_a_stable_release() {
        let dev = card("fs-people");
        assert!(dev.prerelease);
        // name, kind line, badge
        assert_eq!(page_title(&dev, "X".into()).children_builders().len(), 3);
        let stable = card("kanidm");
        assert!(!stable.prerelease);
        assert_eq!(page_title(&stable, "X".into()).children_builders().len(), 2);
    }
}
