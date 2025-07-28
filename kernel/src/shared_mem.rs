use core::sync::atomic::AtomicU64;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use common::AllocPageSize;
use spin::rwlock::RwLock;
use x86_64::structures::paging::PhysFrame;

#[derive(Debug)]
pub struct SharedMem {
    pub size: AllocPageSize,
    pub phys_frames: Box<[PhysFrame]>,
}

pub static NEXT_SHARED_MEM_ID: AtomicU64 = AtomicU64::new(0);
pub static SHARED_MEM: RwLock<BTreeMap<u64, SharedMem>> = RwLock::new(BTreeMap::new());
