use std::cell::Cell;
use std::rc::{Rc, Weak};

use web_sys::KeyboardEvent;

/// One replaceable asynchronous operation owned by a view.
#[derive(Default)]
pub struct OperationSlot {
    current: Option<Rc<Cell<bool>>>,
}

impl OperationSlot {
    /// Invalidate the previous operation and return the new operation's ticket.
    pub fn begin(&mut self) -> Operation {
        self.cancel();
        let active = Rc::new(Cell::new(true));
        let operation = Operation::new(&active);
        self.current = Some(active);
        operation
    }

    /// Invalidate pending results. An underlying browser Promise may continue.
    pub fn cancel(&mut self) {
        if let Some(active) = self.current.take() {
            active.set(false);
        }
    }
}

/// A task may publish only while its ticket is still current.
///
/// The slot holds the sole strong reference, so destroying the owner also
/// invalidates this ticket. Each begin uses a new allocation: no wrapping
/// counter can accidentally revive a previous operation after close/reopen.
#[must_use = "keep the operation ticket and check it before publishing its result"]
pub struct Operation {
    owner: Weak<Cell<bool>>,
    active: Rc<Cell<bool>>,
}

impl Operation {
    fn new(owner: &Rc<Cell<bool>>) -> Self {
        Self {
            owner: Rc::downgrade(owner),
            active: Rc::new(Cell::new(true)),
        }
    }

    pub fn is_current(&self) -> bool {
        self.active.get() && self.owner.upgrade().is_some_and(|owner| owner.get())
    }

    /// Claim the final result once, immediately before applying it to the UI.
    #[must_use]
    pub fn finish(self) -> bool {
        self.is_current() && self.active.replace(false)
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        self.active.set(false);
    }
}

/// Independent tasks that share a view lifetime. Starting an import does not
/// replace another import; resetting the board invalidates all pending results.
/// The scope stores one identity, with no registry of past task tickets.
pub struct OperationScope(Rc<Cell<bool>>);

impl Default for OperationScope {
    fn default() -> Self {
        Self(Rc::new(Cell::new(true)))
    }
}

impl OperationScope {
    pub fn begin(&self) -> Operation {
        Operation::new(&self.0)
    }

    pub fn cancel(&mut self) {
        self.0.set(false);
        self.0 = Rc::new(Cell::new(true));
    }
}

/// A bounded watermark for effects from one ordered authoritative stream.
///
/// IDs must increase for distinct effects. A duplicate or older ID cannot
/// replay its effect. Reset explicitly when the authoritative session changes,
/// or include its epoch in an ordered ID. This does not deduplicate arbitrary
/// unordered event IDs, and stores exactly one ID, regardless of play duration.
pub struct OnceEffects<Id: Ord> {
    last: Option<Id>,
}

impl<Id: Ord> Default for OnceEffects<Id> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<Id: Ord> OnceEffects<Id> {
    #[must_use]
    pub fn claim(&mut self, id: Id) -> bool {
        if self.last.as_ref().is_some_and(|last| &id <= last) {
            return false;
        }
        self.last = Some(id);
        true
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// Shared composition state for an editing scope. Wire `start` and `end` to
/// its composition events, and check `blocks_key` before submit/shortcut keys.
#[derive(Clone, Default)]
pub struct Composition(Rc<Cell<bool>>);

impl Composition {
    pub fn start(&self) {
        self.0.set(true);
    }

    pub fn end(&self) {
        self.0.set(false);
    }

    pub fn is_composing(&self) -> bool {
        self.0.get()
    }

    pub fn blocks_key(&self, event: &KeyboardEvent) -> bool {
        // Some browser/IME combinations emit keyCode 229 at composition end,
        // even though isComposing has already become false.
        self.is_composing() || event.is_composing() || event.key_code() == 229
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaced_cancelled_and_dropped_owners_reject_late_success_or_error() {
        let mut slot = OperationSlot::default();
        let first = slot.begin();
        let second = slot.begin();
        assert!(!first.is_current());
        assert!(!first.finish());
        assert!(second.is_current());
        slot.cancel();
        let reopened = slot.begin();
        assert!(!second.finish(), "close/reopen revived the old operation");
        assert!(reopened.finish());
        let orphan = slot.begin();
        drop(slot);
        assert!(!orphan.finish());
    }

    #[test]
    fn dropped_tasks_cancel_without_invalidating_their_replacement() {
        let mut slot = OperationSlot::default();
        let old = slot.begin();
        let current = slot.begin();
        drop(old);
        assert!(current.is_current());
        assert!(current.finish());
        let abandoned = slot.begin();
        let observed = Rc::clone(&abandoned.active);
        drop(abandoned);
        assert!(!observed.get());
    }

    #[test]
    fn independent_tasks_coexist_but_reset_or_owner_destruction_cancels_all() {
        let mut scope = OperationScope::default();
        let first = scope.begin();
        let second = scope.begin();
        assert!(second.finish());
        assert!(first.finish());
        let old = scope.begin();
        scope.cancel();
        let new = scope.begin();
        assert!(!old.finish());
        assert!(new.finish());
        for _ in 0..1000 {
            assert!(scope.begin().finish());
        }
        assert_eq!(Rc::weak_count(&scope.0), 0, "scope retained old tickets");
        let orphan = scope.begin();
        drop(scope);
        assert!(!orphan.finish());
    }

    #[test]
    fn effects_reject_duplicates_and_old_messages_with_one_watermark() {
        let mut effects = OnceEffects::default();
        assert!(effects.claim((1, 0)));
        assert!(!effects.claim((1, 0)));
        assert!(effects.claim((1, 2)));
        assert!(!effects.claim((1, 1)));
        assert!(effects.claim((2, 0)));
        assert!(!effects.claim((1, 3)));
        for id in 1..100_000 {
            assert!(effects.claim((2, id)));
        }
        assert_eq!(effects.last, Some((2, 99_999)));
        effects.reset();
        assert!(effects.claim((1, 0)));
    }

    #[test]
    fn composition_state_is_shared_by_handlers_and_ends_explicitly() {
        let composition = Composition::default();
        let handler = composition.clone();
        assert!(!composition.is_composing());
        handler.start();
        assert!(composition.is_composing());
        handler.end();
        assert!(!composition.is_composing());
    }
}
