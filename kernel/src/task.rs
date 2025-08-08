use core::{
    num::{NonZero, NonZeroU32},
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::Arc,
    vec::Vec,
};
use common::{SliceData, Syscall, SyscallWaitUntilEvent};
use nodit::{Interval, NoditMap};
use x86_64::structures::paging::PhysFrame;

use crate::{
    CapabilityId, ManagedL4PageTable, interrupted_context::InterruptedContext,
    local_apic_id::LocalApicId, syscall_saved_regs::SyscallSavedRegs,
    try_access_user_mem::try_access_user_mem,
};

#[derive(Debug, Clone)]
pub struct ThreadWaitingState {
    pub saved_regs: SyscallSavedRegs,
    pub events_slice: SliceData,
    pub events: BTreeMap<NonZero<u64>, bool>,
}

impl ThreadWaitingState {
    /// # Safety
    /// Enters user mode according to saved registers
    pub unsafe fn sysretq(self) -> ! {
        // let events = unsafe {
        //     self.events_slice
        //         .to_slice_mut::<MaybeUninit<NonZero<u64>>>()
        // };
        let mut events_count = 0;
        for event in self.events.into_iter().filter_map(
            |(event, happened)| {
                if happened { Some(event) } else { None }
            },
        ) {
            let event_ptr = NonNull::new(
                (self.events_slice.pointer() as usize + events_count * size_of::<NonZero<u64>>())
                    as *mut NonZero<u64>,
            )
            .unwrap();
            let _ = try_access_user_mem(|| {
                unsafe { event_ptr.write(event) };
                Box::new(())
            });
            events_count += 1;
        }
        let output = SyscallWaitUntilEvent::encode_output(&Ok(events_count.try_into().unwrap()));
        unsafe { self.saved_regs.sysretq(output) }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SharedVirtMem {
    pub shared_mem_id: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UserVirtMem {
    /// Memory that is not shared or MMIO, just owned by the process
    Plain,
    /// The framebuffer is MMIO with fixed permissions and cache behavior set by the kernel
    FrameBuffer,
    /// Different processes can have different permissions for the same shared mem
    Shared(SharedVirtMem),
    /// Mapped to Limine module (as read-only)
    LimineModule,
    /// Shared between the process and the kernel.
    EventStream(CapabilityId),
}

pub type ProcessMappedVirtMem = NoditMap<u64, Interval<u64>, UserVirtMem>;

#[derive(Debug)]
pub struct ProcessMemory {
    pub mapped_virtual_memory: ProcessMappedVirtMem,
    pub l4: ManagedL4PageTable,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MutexKey {
    pub process: ProcessId,
    pub virtual_address: u64,
}
