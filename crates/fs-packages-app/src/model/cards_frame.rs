//! The cards of one draw: computed once and shared while a draw is in progress.
//!
//! Pattern: Guard (RAII) - [`FrameCards`] switches the sharing on and, when dropped, off and empties it.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use super::cards::Card;
use super::store::StoreModel;

/// The state of the sharing, inside the model.
#[derive(Debug, Clone, Default)]
pub(super) struct CardFrame {
    /// The cards of the draw in progress; `None` outside a draw.
    pub(super) cache: RefCell<Option<Arc<Vec<Card>>>>,
    /// Is a draw in progress: `cards()` may reuse the cache.
    pub(super) on: Cell<bool>,
    /// How often the cards were computed (for tests of the cache).
    pub(super) builds: Cell<usize>,
}

/// A draw in progress: the cards are computed once while it lives (see [`StoreModel::frame_cards`]).
pub struct FrameCards<'a>(&'a StoreModel);

impl Drop for FrameCards<'_> {
    fn drop(&mut self) {
        self.0.ui.frame.on.set(false);
        *self.0.ui.frame.cache.borrow_mut() = None;
    }
}

impl StoreModel {
    /// The same cards, shared: inside a draw ([`StoreModel::frame_cards`]) computed once and handed out again,
    /// outside always computed fresh. Prefer this where the cards are only read.
    #[must_use]
    pub fn cards_shared(&self) -> std::sync::Arc<Vec<Card>> {
        if self.ui.frame.on.get()
            && let Some(cards) = self.ui.frame.cache.borrow().as_ref()
        {
            return std::sync::Arc::clone(cards);
        }
        let cards = std::sync::Arc::new(self.compute_cards());
        if self.ui.frame.on.get() {
            *self.ui.frame.cache.borrow_mut() = Some(std::sync::Arc::clone(&cards));
        }
        cards
    }

    /// Start a draw: until the returned guard is dropped the cards are computed once. The model must not change
    /// while the guard lives (the draw only reads it).
    #[must_use]
    pub fn frame_cards(&self) -> FrameCards<'_> {
        self.ui.frame.on.set(true);
        *self.ui.frame.cache.borrow_mut() = None;
        FrameCards(self)
    }

    /// How often the cards were computed since the model was made.
    #[must_use]
    pub fn cards_computed(&self) -> usize {
        self.ui.frame.builds.get()
    }
}
