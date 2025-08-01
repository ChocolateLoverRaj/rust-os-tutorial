use core::sync::atomic::{AtomicU8, AtomicUsize};

#[derive(Debug)]
#[repr(C)]
pub struct EventStreamMem {
    pub write_count: AtomicUsize,
    pub read_count: AtomicUsize,
    pub slots: [AtomicU8; 0],
}
