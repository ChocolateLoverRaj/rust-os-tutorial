use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};

use crate::task::Process;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityId(u64);
impl CapabilityId {
    pub fn new_unique() -> Self {
        static NEXT_CAPABILITY_ID: AtomicU64 = AtomicU64::new(0);
        Self(NEXT_CAPABILITY_ID.fetch_add(1, Ordering::Relaxed))
    }
}
impl From<u64> for CapabilityId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<CapabilityId> for u64 {
    fn from(value: CapabilityId) -> Self {
        value.0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CapabilityType {
    /// Allows reading PS/2 keyboard input
    Ps2Keyboard,
    /// Allows accessing a specific shared memory
    SharedMem(u64),
}

#[derive(Debug)]
pub struct Capability {
    pub _type: CapabilityType,
    pub process: Arc<Process>,
}

pub static CAPABILITIES: spin::RwLock<BTreeMap<CapabilityId, Capability>> =
    spin::RwLock::new(BTreeMap::new());
