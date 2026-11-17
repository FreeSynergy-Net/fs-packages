//! The card list reads the rows through an index by name: the first row of a name wins (as the scan did), and
//! every name keeps its own row among many.
//!
//! Pattern: Index (lookup by name) - tests of the behaviour that must not change.

mod common;

use common::{fake, model, package, v};

#[test]
fn the_first_offered_row_of_a_name_wins() {
    let f = fake(Vec::new(), None);
    {
        let mut s = f.state.lock().unwrap();
        s.offered = vec![
            package("fs-theme", v(2, 0, 0)),
            package("fs-theme", v(1, 0, 0)),
        ];
    }
    let m = model(&f);
    let cards = m.cards();
    let theme: Vec<_> = cards.iter().filter(|c| c.name == "fs-theme").collect();
    assert_eq!(theme.len(), 1, "never a name twice");
    assert_eq!(theme[0].offered, Some(v(2, 0, 0)), "the first row wins");
}

#[test]
fn many_names_each_get_their_own_row() {
    let f = fake(Vec::new(), None);
    {
        let mut s = f.state.lock().unwrap();
        s.offered = (0..500)
            .map(|i| package(&format!("fs-app-{i:03}"), v(0, 0, i)))
            .collect();
    }
    let m = model(&f);
    let cards = m.cards();
    assert_eq!(cards.len(), 500);
    for card in &cards {
        let i: u64 = card.name["fs-app-".len()..].parse().unwrap();
        assert_eq!(card.offered, Some(v(0, 0, i)), "{}", card.name);
    }
}
