use core::{
    num::{NonZero, NonZeroU32},
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::boxed::Box;
use common::{
    FUTEX_WAITERS, FutexLockError, FutexUnlockError, LOWER_HALF_END, Syscall, SyscallFutexLock,
    SyscallFutexUnlock,
};
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{
        MutexKey, THREAD_PRIORITIES, THREADS, ThreadId, ThreadReadyState,
        ThreadReadyStateInSyscall, ThreadState, WaitingForMutexState,
    },
    try_access_user_mem::try_access_user_mem,
};

use super::GenericSyscallHandler;

pub struct SyscallFutexLockHandler;
impl GenericSyscallHandler for SyscallFutexLockHandler {
    type S = SyscallFutexLock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            RunThreads,
            Return(<SyscallFutexLock as Syscall>::Output),
        }
        let action = {
            let ptr_u64 = *helper.input();
            if ptr_u64 > 0
                && ptr_u64 + u64::try_from(size_of::<AtomicU64>()).unwrap() <= LOWER_HALF_END
            {
                let ptr = ptr_u64 as *mut AtomicU64;
                let local = get_local();
                let mut running_thread = local.running_thread.try_lock().unwrap();
                let running_thread_id = running_thread.unwrap();
                let threads = THREADS.read();
                let current_thread = threads.get(&running_thread_id).unwrap();
                let mut mutexes = current_thread.process.mutexes.write();
                match try_access_user_mem(|| {
                    let ptr = unsafe { ptr.as_mut() }.unwrap();
                    Box::new(ptr.fetch_or(FUTEX_WAITERS, Ordering::AcqRel))
                }) {
                    Ok(lock_owner) => {
                        if let Some(thread_id) = NonZero::new(*lock_owner as u32) {
                            let thread_id = ThreadId::from_raw(thread_id);
                            if let Some(lock_owner) = threads.get(&thread_id) {
                                mutexes
                                    .entry(MutexKey {
                                        process: lock_owner.process.id,
                                        virtual_address: ptr_u64,
                                    })
                                    .or_default()
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
                                Action::Return(Err(FutexLockError::UnknownLockOwner))
                            }
                        } else {
                            Action::Return(Err(FutexLockError::CheckWithWaiters))
                        }
                    }
                    Err(_e) => Action::Return(Err(FutexLockError::InvalidPointer)),
                }
            } else {
                Action::Return(Err(FutexLockError::InvalidPointer))
            }
        };
        match action {
            Action::RunThreads => run_threads(),
            Action::Return(output) => helper.syscall_return(&output),
        }
    }
}

pub struct SyscallFutexUnlockHandler;
impl GenericSyscallHandler for SyscallFutexUnlockHandler {
    type S = SyscallFutexUnlock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            RunThreads,
            Return(<SyscallFutexUnlock as Syscall>::Output),
        }
        let action = {
            let ptr_u64 = *helper.input();
            if ptr_u64 > 0
                && ptr_u64 + u64::try_from(size_of::<AtomicU64>()).unwrap() <= LOWER_HALF_END
            {
                let local = get_local();
                let mut running_thread = local.running_thread.try_lock().unwrap();
                let current_thread_id = running_thread.unwrap();
                let threads = THREADS.read();
                let current_thread = threads.get(&current_thread_id).unwrap();
                let mut mutexes = current_thread.process.mutexes.write();

                match mutexes.get_mut(&MutexKey {
                    process: current_thread.process.id,
                    virtual_address: ptr_u64,
                }) {
                    Some(mutex) => {
                        let mut waiters = mutex.waiters.lock();
                        let thread_priorities = THREAD_PRIORITIES.read();
                        let highest_priority_waiter = thread_priorities
                            .iter()
                            .find(|thread_id| waiters.contains(thread_id))
                            .unwrap();
                        *current_thread.state.write() = ThreadState::Ready(
                            ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                                saved_regs: helper.saved_regs().clone(),
                                output: Self::S::encode_output(&Ok(())),
                            }),
                        );
                        *running_thread = None;
                        let mut new_lock_owner_state =
                            threads.get(highest_priority_waiter).unwrap().state.write();
                        if let ThreadState::WaitingForMutex(data) = new_lock_owner_state.deref() {
                            let new_state = ThreadState::Ready(ThreadReadyState::InSyscall(
                                ThreadReadyStateInSyscall {
                                    saved_regs: data.saved_regs.clone(),
                                    output: SyscallFutexLock::encode_output(&Ok(())),
                                },
                            ));
                            *new_lock_owner_state = new_state;
                            let mut local_apic = local.local_apic.get().unwrap().lock();
                            waiters.remove(highest_priority_waiter);
                            let thread_only =
                                u64::from(u32::from(NonZeroU32::from(*highest_priority_waiter)));
                            let ptr = ptr_u64 as *mut AtomicU64;
                            // TODO: Maybe don't ignore errors
                            let _ = try_access_user_mem(|| {
                                let ptr = unsafe { ptr.as_ref() }.unwrap();
                                ptr.store(
                                    if waiters.is_empty() {
                                        thread_only
                                    } else {
                                        thread_only | FUTEX_WAITERS
                                    },
                                    Ordering::Release,
                                );
                                Box::new(())
                            });
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
                    None => Action::Return(Ok(())),
                }
            } else {
                Action::Return(Err(FutexUnlockError::InvalidPointer))
            }
        };
        match action {
            Action::RunThreads => run_threads(),
            Action::Return(output) => helper.syscall_return(&output),
        }
    }
}
