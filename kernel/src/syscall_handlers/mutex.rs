use core::{
    num::NonZeroU32,
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

use common::{SyscallFutexLock, SyscallFutexUnlock};
use nodit::Interval;
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{
        MUTEXES, MutexKey, THREAD_PRIORITIES, THREADS, ThreadId, ThreadReadyState, ThreadState,
        WaitingForMutexState,
    },
};

use super::GenericSyscallHandler;

const FUTEX_WAITERS: u64 = 1 << 63;

pub struct SyscallAquireLockHandler;
impl GenericSyscallHandler for SyscallAquireLockHandler {
    type S = SyscallFutexLock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            RunThreads,
            Return,
            Terminate,
        }
        let action = {
            let ptr_u64 = helper.input().clone();
            let local = get_local();
            let mut running_thread = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread.unwrap();
            let threads = THREADS.read();
            let current_thread = threads.get(&running_thread_id).unwrap();
            if let Some(end) = ptr_u64.checked_add(size_of::<AtomicU64>() as u64) {
                let interval = Interval::from(ptr_u64..=end);
                let virtual_memory = current_thread.process.mapped_virtual_memory.read();
                if virtual_memory.contains_interval(interval)
                    && virtual_memory
                        .overlapping(interval)
                        .all(|(_, permissions)| permissions.write)
                {
                    let ptr = ptr_u64 as *mut AtomicU64;
                    if ptr.is_aligned() {
                        let a = unsafe { ptr.as_ref() }.unwrap();
                        let mut mutexes = MUTEXES.write();
                        let lock_owner = a.fetch_or(FUTEX_WAITERS, Ordering::AcqRel);
                        if let Some(thread_id) = NonZeroU32::new(lock_owner as u32) {
                            let thread_id = ThreadId::from_raw(thread_id);
                            if let Some(lock_owner) = threads.get(&thread_id) {
                                mutexes
                                    .entry(MutexKey {
                                        process: lock_owner.process.id,
                                        virtual_address: ptr_u64,
                                    })
                                    .or_insert_with(Default::default)
                                    .waiters
                                    .lock()
                                    .insert(running_thread_id);
                                *current_thread.state.write() =
                                    ThreadState::WaitingForMutex(WaitingForMutexState {
                                        saved_regs: helper.saved_regs().clone(),
                                    });
                                *running_thread = None;
                                Action::RunThreads
                            } else {
                                Action::Return
                            }
                        } else {
                            Action::Return
                        }
                    } else {
                        Action::Terminate
                    }
                } else {
                    Action::Terminate
                }
            } else {
                Action::Terminate
            }
        };
        match action {
            Action::RunThreads => run_threads(),
            Action::Return => helper.syscall_return(&()),
            Action::Terminate => todo!(),
        }
    }
}

pub struct SyscallReleaseLockHandler;
impl GenericSyscallHandler for SyscallReleaseLockHandler {
    type S = SyscallFutexUnlock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Terminate,
            RunThreads,
            Return,
        }
        let action = {
            // log::debug!("releasing lock");
            let address = helper.input().clone();
            // TODO: Validate address
            let mut mutexes = MUTEXES.write();
            let local = get_local();
            let mut running_thread = local.running_thread.try_lock().unwrap();
            let current_thread_id = running_thread.unwrap();
            let threads = THREADS.read();
            let current_thread = threads.get(&current_thread_id).unwrap();

            match mutexes.get_mut(&MutexKey {
                process: current_thread.process.id,
                virtual_address: address,
            }) {
                Some(mutex) => {
                    let mut waiters = mutex.waiters.lock();
                    let thread_priorities = THREAD_PRIORITIES.read();
                    let highest_priority_waiter = thread_priorities
                        .iter()
                        .find(|thread_id| waiters.contains(thread_id))
                        .unwrap();
                    *current_thread.state.write() = ThreadState::Ready(
                        ThreadReadyState::InSyscall(helper.saved_regs().clone()),
                    );
                    *running_thread = None;
                    let mut new_lock_owner_state =
                        threads.get(highest_priority_waiter).unwrap().state.write();
                    if let ThreadState::WaitingForMutex(data) = new_lock_owner_state.deref() {
                        let new_state = ThreadState::Ready(ThreadReadyState::InSyscall(
                            data.saved_regs.clone(),
                        ));
                        *new_lock_owner_state = new_state;
                        let mut local_apic = local.local_apic.get().unwrap().lock();
                        waiters.remove(highest_priority_waiter);
                        let thread_only =
                            u64::from(u32::from(NonZeroU32::from(*highest_priority_waiter)));
                        let a = address as *const AtomicU64;
                        let a = unsafe { a.as_ref() }.unwrap();
                        a.store(
                            if waiters.is_empty() {
                                thread_only
                            } else {
                                thread_only | FUTEX_WAITERS
                            },
                            Ordering::Release,
                        );
                        unsafe {
                            local_apic.send_ipi_all(
                                u8::from(InterruptVector::CheckTasks),
                                IpiAllShorthand::AllExcludingSelf,
                            );
                        }
                        Action::RunThreads
                    } else {
                        unreachable!()
                    }
                }
                None => Action::Return,
            }
        };
        match action {
            Action::RunThreads => run_threads(),
            Action::Terminate => {
                todo!()
            }
            Action::Return => helper.syscall_return(&()),
        }
    }
}
