//! A finished icon round computes the kept cards again (the icon files are not part of the rows, so without this the
//! tiles would keep their letters after the icons arrived).

mod common;

use common::{fake, model};

#[test]
fn a_finished_icon_round_computes_the_cards_again() {
    let f = fake(Vec::new(), None);
    let mut m = model(&f);
    let base = m.cards_computed();
    let _ = m.cards();
    assert_eq!(m.cards_computed() - base, 1);
    let _ = m.cards();
    assert_eq!(m.cards_computed() - base, 1, "nothing changed: kept");
    m.add_icon_problems(Vec::new());
    let _ = m.cards();
    assert_eq!(
        m.cards_computed() - base,
        2,
        "icon round done: computed again"
    );
}
