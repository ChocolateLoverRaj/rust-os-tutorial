use core::{cell::RefCell, task::Waker};

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use futures::task::AtomicWaker;

#[derive(Debug, Default)]
pub struct ExecutorContext {
    events: RefCell<BTreeMap<u64, AtomicWaker>>,
}

impl ExecutorContext {
    pub fn register_waker(&self, event_id: u64, waker: &Waker) {
        self.events
            .borrow_mut()
            .entry(event_id)
            .or_default()
            .register(waker);
    }

    /// Returns the total number of events that we're listening to
    pub fn events(&self) -> Box<[u64]> {
        self.events.borrow().keys().copied().collect()
    }

    pub fn wake(&self, event_id: u64) {
        self.events.borrow().get(&event_id).unwrap().wake();
    }
}
