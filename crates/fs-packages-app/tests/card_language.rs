//! The language is part of what a card is made of: another language computes the cards again. Own test file: it changes the environment.

mod common;

use common::{fake, model, package, v};

#[test]
fn the_language_computes_the_cards_again() {
    use fs_packages_core::Kind;
    let f = fake(Vec::new(), None);
    f.state.lock().unwrap().offered = vec![{
        let mut p = package("fs-style", v(1, 0, 0));
        p.kind = Kind::Theme;
        p
    }];
    let m = model(&f);
    // SAFETY: this test file has this one test that changes the environment, and no thread of it reads it meanwhile.
    unsafe { std::env::set_var("FS_LANG", "en") };
    let en = m.cards();
    assert_eq!(m.cards().len(), en.len());
    let base = m.cards_computed();
    let _ = m.cards();
    assert_eq!(m.cards_computed(), base, "same language: kept");
    unsafe { std::env::set_var("FS_LANG", "de") };
    let de = m.cards();
    assert_eq!(
        m.cards_computed() - base,
        1,
        "other language: computed again"
    );
    assert_ne!(
        en[0].open_blocked, de[0].open_blocked,
        "and the words are German"
    );
}
