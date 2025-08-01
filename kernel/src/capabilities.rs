use core::{
    num::NonZero,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};

use crate::{
    event_stream_mem::EventStreamMem, task::Process, try_access_user_mem::try_access_user_mem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityId(NonZero<u64>);
impl CapabilityId {
    pub fn new_unique() -> Self {
        static NEXT_CAPABILITY_ID: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT_CAPABILITY_ID
                .fetch_add(1, Ordering::Relaxed)
                .try_into()
                .unwrap(),
        )
    }
}
impl From<NonZero<u64>> for CapabilityId {
    fn from(value: NonZero<u64>) -> Self {
        Self(value)
    }
}
impl From<CapabilityId> for NonZero<u64> {
    fn from(value: CapabilityId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub enum ChannelAccess {
    Send,
    Receive,
}

#[derive(Debug)]
pub struct Channel {
    pub pending: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct ChannelCapability {
    pub access: ChannelAccess,
    pub channel: Arc<Channel>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EventStreamSource {
    Ps2Keyboard,
    Ps2Mouse,
}

#[derive(Debug)]
pub struct EventStream {
    pub process: Arc<Process>,
    pub source: EventStreamSource,
    pub ptr: usize,
    pub slots_len: usize,
}

#[derive(Debug)]
pub enum CapabilityType {
    /// Allows reading PS/2 keyboard input
    Ps2Keyboard,
    /// Allows accessing a specific shared memory
    SharedMem(u64),
    Channel(Arc<AtomicBool>),
    EventStream(EventStream),
}

impl CapabilityType {
    /// Clones if the capability is allowed to be cloned so that multiple processes can access it. Otherwise, it doesn't clone it.
    pub fn try_clone(&self) -> Option<Self> {
        match self {
            Self::Ps2Keyboard => Some(Self::Ps2Keyboard),
            Self::SharedMem(id) => Some(Self::SharedMem(*id)),
            Self::Channel(arc) => Some(Self::Channel(arc.clone())),
            Self::EventStream(_) => None,
        }
    }

    pub fn can_send(&self) -> bool {
        match self {
            Self::Ps2Keyboard => true,
            Self::SharedMem(_) => true,
            Self::Channel(_) => true,
            // Event streams contain a pointer to virtual memory specific to a process, so it cannot be sent.
            // The keyboard capability can be sent, allowing other processes to create their own event streams.
            Self::EventStream(_) => false,
        }
    }

    /// Returns `None` if the capability does not act as an event.
    pub fn take_pending_event(&self) -> Option<bool> {
        match self {
            Self::EventStream(event_stream) => Some({
                let mem = NonNull::new(event_stream.ptr as *mut EventStreamMem).unwrap();
                try_access_user_mem(|| {
                    let mem = unsafe { mem.as_ref() };
                    Box::new(
                        mem.read_count.load(Ordering::Relaxed)
                            < mem.write_count.load(Ordering::Relaxed),
                    )
                })
                .is_ok_and(|b| *b)
            }),
            CapabilityType::Channel(pending) => Some(pending.swap(false, Ordering::Relaxed)),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Capability {
    pub _type: CapabilityType,
    pub process_id: NonZero<u32>,
}

pub static CAPABILITIES: spin::RwLock<BTreeMap<NonZero<u64>, Capability>> =
    spin::RwLock::new(BTreeMap::new());
