use core::{
    mem::MaybeUninit,
    num::NonZeroU32,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::Arc,
    vec::Vec,
};
use common::{SliceData, SpawnProcessMemoryFlags, Syscall, SyscallWaitUntilEvent};
use crossbeam_queue::ArrayQueue;
use nodit::{Interval, NoditMap};
use x86_64::structures::paging::PhysFrame;

use crate::{
    elf_segment_flags::ElfSegmentFlags, interrupted_context::InterruptedContext,
    local_apic_id::LocalApicId, syscall_saved_regs::SyscallSavedRegs,
};

/// Read is always given, because it doesn't make sense not to have read
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VirtualMemoryPermissions {
    /// If this is fault, this means that the page is intentionally left unmapped.
    /// This is useful for stack guard pages
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl From<ElfSegmentFlags> for VirtualMemoryPermissions {
    fn from(value: ElfSegmentFlags) -> Self {
        Self {
            read: value.contains(ElfSegmentFlags::READABLE),
            write: value.contains(ElfSegmentFlags::WRITABLE),
            execute: value.contains(ElfSegmentFlags::EXECUTABLE),
        }
    }
}

impl From<SpawnProcessMemoryFlags> for VirtualMemoryPermissions {
    fn from(value: SpawnProcessMemoryFlags) -> Self {
        Self {
            read: value.contains(SpawnProcessMemoryFlags::READABLE),
            write: value.contains(SpawnProcessMemoryFlags::WRITABLE),
            execute: value.contains(SpawnProcessMemoryFlags::EXECUTABLE),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadWaitingState {
    pub saved_regs: SyscallSavedRegs,
    pub events_slice: SliceData,
    pub events: BTreeMap<u64, bool>,
}

impl ThreadWaitingState {
    /// # Safety
    /// Enters user mode according to saved registers
    pub unsafe fn sysretq(self) -> ! {
        let events = unsafe { self.events_slice.to_slice_mut::<MaybeUninit<u64>>() };
        let mut events_count = 0;
        for event in self.events.into_iter().filter_map(
            |(event, happened)| {
                if happened { Some(event) } else { None }
            },
        ) {
            events[events_count].write(event);
            events_count += 1;
        }
        let output = SyscallWaitUntilEvent::encode_output(&(events_count as u64));
        unsafe { self.saved_regs.sysretq(output) }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EventStreamSource {
    Ps2Keyboard,
    Ps2Mouse,
}

#[derive(Debug)]
pub struct EventStream {
    pub process: ProcessId,
    pub source: EventStreamSource,
    pub queue: ArrayQueue<u8>,
}

pub static EVENT_ID: AtomicU64 = AtomicU64::new(0);

pub type ProcessMappedVirtMem = NoditMap<u64, Interval<u64>, VirtualMemoryPermissions>;

#[derive(Debug)]
pub struct ProcessMemory {
    pub mapped_virtual_memory: ProcessMappedVirtMem,
    pub frame_buffer_virtual_start: Option<u64>,
}

#[derive(Debug)]
pub struct Process {
    pub id: ProcessId,
    pub cr3: PhysFrame,
    pub memory: spin::RwLock<ProcessMemory>,
    pub mutexes: spin::RwLock<BTreeMap<MutexKey, UserMutex>>,
}

#[derive(Debug, Clone)]
pub struct StartData {
    pub rip: u64,
    pub rsp: u64,
}

#[derive(Debug, Clone)]
pub struct ThreadReadyStateInSyscall {
    pub saved_regs: SyscallSavedRegs,
    pub output: [u64; 7],
}

impl ThreadReadyStateInSyscall {
    /// # Safety
    /// Enters user mode
    pub unsafe fn sysretq(self) -> ! {
        unsafe { self.saved_regs.sysretq(self.output) }
    }
}

#[derive(Debug, Clone)]
pub enum ThreadReadyState {
    ReadyToStart(StartData),
    Interrupted(InterruptedContext),
    InSyscall(ThreadReadyStateInSyscall),
}

#[derive(Debug, Default)]
pub struct UserMutex {
    pub waiters: spin::Mutex<BTreeSet<ThreadId>>,
}

#[derive(Debug)]
pub struct WaitingForMutexState {
    pub saved_regs: SyscallSavedRegs,
    // pub mutex_id: u64,
}

#[derive(Debug)]
pub enum ThreadState {
    Ready(ThreadReadyState),
    Running(LocalApicId),
    /// A thread could be in this state even if it is ready (which is if at least 1 event which it is waiting for happened)
    WaitingForEvents(ThreadWaitingState),
    WaitingForMutex(WaitingForMutexState),
}

impl ThreadState {
    pub fn is_ready(&self) -> bool {
        match self {
            ThreadState::Ready(_) => true,
            ThreadState::Running(_) => false,
            Self::WaitingForEvents(state) => state.events.values().any(|happened| *happened),
            Self::WaitingForMutex(_) => false,
        }
    }
}

#[derive(Debug)]
pub struct Thread {
    pub state: spin::RwLock<ThreadState>,
    pub process: Arc<Process>,
}

static NEXT_THREAD_ID: AtomicU32 = AtomicU32::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(NonZeroU32);
impl ThreadId {
    pub fn new_unique() -> Self {
        Self(
            NEXT_THREAD_ID
                .fetch_add(1, Ordering::Relaxed)
                .try_into()
                .unwrap(),
        )
    }

    pub fn from_raw(thread_id: NonZeroU32) -> Self {
        Self(thread_id)
    }
}
impl From<ThreadId> for NonZeroU32 {
    fn from(value: ThreadId) -> Self {
        value.0
    }
}

static NEXT_PROCESS_ID: AtomicU32 = AtomicU32::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(NonZeroU32);
impl ProcessId {
    pub fn new_unique() -> Self {
        Self(
            NEXT_PROCESS_ID
                .fetch_add(1, Ordering::Relaxed)
                .try_into()
                .unwrap(),
        )
    }
}
impl From<ProcessId> for NonZeroU32 {
    fn from(value: ProcessId) -> Self {
        value.0
    }
}

pub static THREAD_PRIORITIES: spin::RwLock<Vec<ThreadId>> = spin::RwLock::new(Vec::new());
pub static THREADS: spin::RwLock<BTreeMap<ThreadId, Thread>> = spin::RwLock::new(BTreeMap::new());

pub static PS2_EVENT_STREAMS: spin::RwLock<BTreeMap<u64, EventStream>> =
    spin::RwLock::new(BTreeMap::new());

#[derive(Debug)]
pub struct Channel {
    pub receiver: ProcessId,
    pub sender: ProcessId,
    pub pending_event: AtomicBool,
}

pub static CHANNELS: spin::RwLock<BTreeMap<u64, Channel>> = spin::RwLock::new(BTreeMap::new());

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MutexKey {
    pub process: ProcessId,
    pub virtual_address: u64,
}
