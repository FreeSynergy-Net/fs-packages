//! The cards kept between draws: they are a function of the model state and the language, never of the width.
//!
//! Pattern: Memo (cache keyed by a fingerprint of the inputs). A resize only lays the same cards out again; the cards are
//! computed again only when something the cards are made of changed (catalog, installed programs, jobs, hover, selection,
//! notes of started programs, problems, language).

use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::Hasher;
use std::sync::Arc;

use super::cards::Card;
use super::store::StoreModel;

/// The kept cards, inside the model.
#[derive(Debug, Clone, Default)]
pub(super) struct CardMemo {
    /// The fingerprint of the inputs the kept cards were made from, and the cards.
    pub(super) kept: RefCell<Option<(u64, Arc<Vec<Card>>)>>,
    /// How often the cards were computed (for tests of the cache).
    pub(super) builds: Cell<usize>,
}

/// Writes text straight into a hasher (no string is built).
struct Sink(DefaultHasher);

impl std::fmt::Write for Sink {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

impl StoreModel {
    /// The fingerprint of everything a card is made of. NOT the width or the height: those only lay the cards out.
    /// Fields of the model the cards do not read (filter text of the search is applied by `listing()`, after the cards) stay out.
    fn cards_fingerprint(&self) -> u64 {
        let mut sink = Sink(DefaultHasher::new());
        // Debug text of the inputs: a new field of a row changes the fingerprint without anyone remembering to add it.
        let _ = write!(
            sink,
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}",
            self.available,
            self.installed,
            self.offers,
            self.jobs,
            self.problems,
            self.fresh,
            (
                &self.ui.hovered,
                &self.ui.selected,
                &self.ui.confirm,
                &self.ui.removing
            ),
            self.ui.launch,
            self.catalog_read,
            fs_packages_core::language::from_environment(),
        );
        sink.0.finish()
    }

    /// The same cards, shared: kept between draws and computed again only when the model (or the language) changed.
    #[must_use]
    pub fn cards_shared(&self) -> Arc<Vec<Card>> {
        let key = self.cards_fingerprint();
        if let Some((kept_key, cards)) = self.ui.memo.kept.borrow().as_ref()
            && *kept_key == key
        {
            return Arc::clone(cards);
        }
        let cards = Arc::new(self.compute_cards());
        *self.ui.memo.kept.borrow_mut() = Some((key, Arc::clone(&cards)));
        cards
    }

    /// How often the cards were computed since the model was made.
    #[must_use]
    pub fn cards_computed(&self) -> usize {
        self.ui.memo.builds.get()
    }
}
