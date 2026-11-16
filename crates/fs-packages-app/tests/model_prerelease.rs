//! The "Pre-release" / "Vorabversion" mark: the card and the page carry it for a pre-release only, and the word has both languages.

mod common;

use common::{fake, model, package, v};

#[test]
fn prerelease_card_and_page_say_so_and_a_stable_one_does_not() {
    let f = fake(Vec::new(), None);
    {
        let mut s = f.state.lock().unwrap();
        s.offered.clear();
        let mut dev = package("fs-dev", v(0, 0, 3));
        dev.details.prerelease = true;
        s.offered.push(dev);
        s.offered.push(package("fs-stable", v(1, 0, 0)));
    }
    let m = model(&f);
    let cards = m.cards();
    let by = |n: &str| cards.iter().find(|c| c.name == n).unwrap();
    assert!(by("fs-dev").prerelease);
    assert!(!by("fs-stable").prerelease);
    assert!(m.package_page("fs-dev").unwrap().card.prerelease);
    assert!(!m.package_page("fs-stable").unwrap().card.prerelease);
}

#[test]
fn prerelease_word_is_translated() {
    use fs_packages_app::words_front::{Lang, prerelease};
    assert_eq!(prerelease(Lang::De), "Vorabversion");
    assert_eq!(prerelease(Lang::En), "Pre-release");
}
