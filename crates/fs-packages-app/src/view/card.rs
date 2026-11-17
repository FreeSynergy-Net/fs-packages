//! Pattern: Adapter (the logo of a package: the stored icon, or a rounded square of its accent colour with the first letter; both have the
//! size asked for). The tiles and the table rows are in `front/`.

use fs_components::blinc::core::{Brush, Color};
use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::layout::image::img;
use fs_components::blinc::layout::prelude::text;

use crate::model::Card;

/// Side of the logo square, in pixels.
const LOGO: f32 = 56.0;
/// Corner radius of the logo square.
const LOGO_RADIUS: f32 = 14.0;

/// The tone of the pre-release badge: neutral (`Quiet`), never the amber of "update available" (`Warn`), so the two marks stay apart.
fn prerelease_tone() -> fs_components_widgets::craft::BadgeTone {
    fs_components_widgets::craft::BadgeTone::Quiet
}

/// The badge "Pre-release" / "Vorabversion" of a package whose offered release is one (`<release type="development">`); `None` otherwise.
pub(super) fn prerelease_badge(card: &Card) -> Option<Div> {
    use fs_components_widgets::craft::badge;
    card.prerelease.then(|| {
        div().flex_row().child(badge(
            crate::words_front::prerelease(crate::words_front::Lang::current()),
            prerelease_tone(),
        ))
    })
}

/// The same mark as a neutral (secondary-text) line in the compact window.
pub(super) fn prerelease_note(card: &Card) -> Option<Div> {
    card.prerelease.then(|| {
        div().child(
            text(crate::words_front::prerelease(
                crate::words_front::Lang::current(),
            ))
            .class("fs-c-quiet"),
        )
    })
}

/// The logo at table size.
pub(super) fn logo_small(card: &Card) -> Div {
    sized_logo(card, 36.0, 9.0, 18.0)
}

/// The logo: the stored icon (a PNG) when there is one, else a rounded square in the accent colour of the name with
/// its first letter in black. Both have the same size.
pub(super) fn logo(card: &Card) -> Div {
    sized_logo(card, LOGO, LOGO_RADIUS, 28.0)
}

/// The logo `side` pixels square with the given corner and letter size.
pub(super) fn sized_logo(card: &Card, side: f32, radius: f32, letter: f32) -> Div {
    let frame = div()
        .w(side)
        .h(side)
        .flex_shrink_0()
        .rounded(radius)
        .overflow_clip();
    if let Some(path) = &card.icon {
        return frame.child(
            img(path.to_string_lossy().into_owned())
                .size(side, side)
                .cover(),
        );
    }
    let accent = fs_components_styles::styles::craft::ACCENTS
        [fs_components_styles::styles::craft::accent_index(&card.name)];
    frame
        .background(Brush::Solid(Color::rgb(
            f32::from(accent.r) / 255.0,
            f32::from(accent.g) / 255.0,
            f32::from(accent.b) / 255.0,
        )))
        .items_center()
        .justify_center()
        .child(
            text(card.placeholder.letter.to_string())
                .size(letter)
                .bold()
                .color(Color::BLACK),
        )
}

/// The logo of variant C: the stored icon, or a rounded square in the colour of the name with its first letter in white.
pub(super) fn logo_plain(card: &Card, side: f32) -> Div {
    let frame = div()
        .w(side)
        .h(side)
        .flex_shrink_0()
        .rounded(side / 6.0)
        .overflow_clip();
    if let Some(path) = &card.icon {
        return frame.child(
            img(path.to_string_lossy().into_owned())
                .size(side, side)
                .cover(),
        );
    }
    let (r, g, b) = card.placeholder.rgb;
    frame
        .background(Brush::Solid(Color::rgb(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        )))
        .items_center()
        .justify_center()
        .child(
            text(card.placeholder.letter.to_string())
                .size(side * 0.5)
                .bold()
                .color(Color::WHITE),
        )
}

#[cfg(test)]
pub(crate) mod tests {
    use fs_components_widgets::craft::BadgeTone;

    use crate::model::{Card, StoreModel};

    /// A card of the demo store (`fs-people` is a pre-release, `kanidm` is stable).
    pub(crate) fn demo_card(name: &str) -> Card {
        let p = crate::demo::parts(true);
        let mut m = StoreModel::new(p.backend).with_updates(p.updates);
        m.refresh();
        m.cards().into_iter().find(|c| c.name == name).unwrap()
    }

    /// A pre-release card has the badge, a stable one has none (the badge of tile and page both come from here).
    #[test]
    fn the_badge_exists_for_a_prerelease_and_not_for_a_stable_release() {
        assert!(super::prerelease_badge(&demo_card("fs-people")).is_some());
        assert!(super::prerelease_badge(&demo_card("kanidm")).is_none());
        assert!(super::prerelease_note(&demo_card("fs-people")).is_some());
        assert!(super::prerelease_note(&demo_card("kanidm")).is_none());
    }

    /// The pre-release badge is neutral, not the amber of "update available".
    #[test]
    fn prerelease_badge_is_quiet_not_warn() {
        assert!(matches!(super::prerelease_tone(), BadgeTone::Quiet));
        assert!(!matches!(super::prerelease_tone(), BadgeTone::Warn));
    }
}
