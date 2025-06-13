use core::sync::atomic::AtomicU64;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use common::SliceData;
use crossbeam_queue::ArrayQueue;
use nodit::{Interval, NoditMap};
use x86_64::structures::paging::PhysFrame;

use crate::{elf_segment_flags::ElfSegmentFlags, syscall_saved_regs::SyscallSavedRegs};

/// Read is always given, because it doesn't make sense not to have read
#[derive(Debug, PartialEq, Eq)]
pub struct VirtualMemoryPermissions {
    pub write: bool,
    pub execute: bool,
}

impl From<ElfSegmentFlags> for VirtualMemoryPermissions {
    fn from(value: ElfSegmentFlags) -> Self {
        Self {
            write: value.contains(ElfSegmentFlags::WRITABLE),
            execute: value.contains(ElfSegmentFlags::EXECUTABLE),
        }
    }
}

pub struct WaitingState {
    pub events: Box<[u64]>,
    pub saved_regs: SyscallSavedRegs,
    pub events_slice: SliceData,
}

pub enum TaskState {
    Running,
    Waiting(WaitingState),
}

#[derive(Debug, PartialEq, Eq)]
pub enum EventStreamSource {
    Ps2Keyboard,
    Ps2Mouse,
}

pub struct EventStream {
    pub source: EventStreamSource,
    pub queue: ArrayQueue<u8>,
    /// Event happened, but syscall wait event was not called
    pub pending_event: bool,
}

pub static EVENT_ID: AtomicU64 = AtomicU64::new(0);

pub struct Task {
    pub cr3: PhysFrame,
    pub mapped_virtual_memory: NoditMap<u64, Interval<u64>, VirtualMemoryPermissions>,
    pub state: TaskState,
    pub event_streams: BTreeMap<u64, EventStream>,
}

pub static TASK: spin::Mutex<Option<Task>> = spin::Mutex::new(None);
