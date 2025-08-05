use core::sync::atomic::{AtomicU8, AtomicUsize};

#[derive(Debug)]
#[repr(C)]
pub struct EventStreamMem {
    pub write_count: AtomicUsize,
    pub read_count: AtomicUsize,
    pub slots: [AtomicU8; 0],
}

impl EventStreamMem {
    pub fn size(slots_len: usize) -> usize {
        size_of::<Self>() + size_of::<AtomicU8>() * slots_len
    }
}
