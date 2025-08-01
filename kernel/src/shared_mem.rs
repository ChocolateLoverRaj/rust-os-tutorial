use core::sync::atomic::AtomicU64;

use alloc::collections::btree_map::BTreeMap;
use common::AllocPageSize;
use nodit::{Interval, NoditSet};
use spin::rwlock::RwLock;

#[derive(Debug)]
pub struct SharedMem {
    pub page_size: AllocPageSize,
    pub phys_mem: NoditSet<u64, Interval<u64>>,
}

pub static NEXT_SHARED_MEM_ID: AtomicU64 = AtomicU64::new(0);
pub static SHARED_MEM: RwLock<BTreeMap<u64, SharedMem>> = RwLock::new(BTreeMap::new());
