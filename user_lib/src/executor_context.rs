use core::{cell::RefCell, num::NonZero, task::Waker};

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use futures::task::AtomicWaker;

#[derive(Debug, Default)]
struct Event {
    waker: AtomicWaker,
    happened: bool,
}

#[derive(Debug, Default)]
pub struct ExecutorContext {
    events: RefCell<BTreeMap<NonZero<u64>, Event>>,
}

impl ExecutorContext {
    pub fn register_waker(&self, event_id: NonZero<u64>, waker: &Waker) {
        self.events
            .borrow_mut()
            .entry(event_id)
            .or_default()
            .waker
            .register(waker);
    }

    pub fn event_not_happened(&self) -> Box<[NonZero<u64>]> {
        self.events
            .borrow()
            .iter()
            .filter_map(|(event_id, event)| {
                if !event.happened {
                    Some(*event_id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn wake(&self, event_id: NonZero<u64>) {
        let mut events = self.events.borrow_mut();
        let event = events.get_mut(&event_id).unwrap();
        event.waker.wake();
        event.happened = true;
    }

    /// Returns true if the event happened, removing the event from the map
    pub fn take(&self, event_id: NonZero<u64>) -> bool {
        self.events
            .borrow_mut()
            .remove(&event_id)
            .is_some_and(|event| event.happened)
    }
}
