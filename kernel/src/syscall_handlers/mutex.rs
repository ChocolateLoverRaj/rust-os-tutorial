use core::ops::Deref;

use alloc::collections::btree_map::Entry;
use common::{SyscallAquireLock, SyscallReleaseLock, SyscallTryAquireLock};

use crate::{
    cpu_local_data::get_local,
    run_tasks::run_threads,
    task::{
        MUTEXES, THREAD_PRIORITIES, THREADS, ThreadReadyState, ThreadState, UserMutex,
        WaitingForMutexState,
    },
};

use super::GenericSyscallHandler;

pub struct SyscallTryAquireLockhandler;
impl GenericSyscallHandler for SyscallTryAquireLockhandler {
    type S = SyscallTryAquireLock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let r = {
            let id = helper.input().clone();
            let mut mutexes = MUTEXES.write();
            let local = get_local();
            let thread_id = local.running_thread.lock().unwrap();
            match mutexes.entry(id) {
                Entry::Occupied(_) => false,
                Entry::Vacant(entry) => {
                    entry.insert(UserMutex {
                        locked_by: thread_id,
                    });
                    true
                }
            }
        };
        helper.syscall_return(&r)
    }
}

pub struct SyscallAquireLockHandler;
impl GenericSyscallHandler for SyscallAquireLockHandler {
    type S = SyscallAquireLock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            RunThreads,
            Return,
        }
        let action = {
            let id = helper.input().clone();
            let mut mutexes = MUTEXES.write();
            let local = get_local();
            let mut running_thread = local.running_thread.lock();
            let thread_id = running_thread.unwrap();
            match mutexes.entry(id) {
                Entry::Occupied(mut entry) => {
                    let locked_by = entry.get_mut().locked_by;
                    if locked_by == thread_id {
                        todo!()
                    }
                    let threads = THREADS.read();
                    let thread = threads.get(&thread_id).unwrap();
                    *thread.state.write() = ThreadState::WaitingForMutex(WaitingForMutexState {
                        saved_regs: helper.saved_regs().clone(),
                        mutex_id: id,
                    });
                    *running_thread = None;
                    log::debug!(
                        "thread: {thread_id:?}. lock held by a different thread. not giving lock"
                    );
                    Action::RunThreads
                }
                Entry::Vacant(entry) => {
                    log::debug!("vacant. giving lock");
                    entry.insert(UserMutex {
                        locked_by: thread_id,
                    });
                    Action::Return
                }
            }
        };
        match action {
            Action::RunThreads => run_threads(),
            Action::Return => helper.syscall_return(&()),
        }
    }
}

pub struct SyscallReleaseLockHandler;
impl GenericSyscallHandler for SyscallReleaseLockHandler {
    type S = SyscallReleaseLock;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Terminate,
            RunThreads,
            Return,
        }
        let action = {
            // log::debug!("releasing lock");
            let id = helper.input().clone();
            let mut mutexes = MUTEXES.write();
            match mutexes.get_mut(&id) {
                Some(mutex) => {
                    let local = get_local();
                    let mut running_thread = local.running_thread.lock();
                    let thread_id = running_thread.unwrap();
                    if mutex.locked_by == thread_id {
                        let thread_priorities = THREAD_PRIORITIES.read();
                        let threads = THREADS.read();
                        let thread = threads.get(&thread_id).unwrap();
                        *thread.state.write() = ThreadState::Ready(ThreadReadyState::InSyscall(
                            helper.saved_regs().clone(),
                        ));
                        let mut thread_priorities = thread_priorities.iter();
                        if let Some(action) = loop {
                            match thread_priorities.next() {
                                Some(thread_id) => {
                                    let thread = threads.get(thread_id).unwrap();
                                    let state = thread.state.upgradeable_read();
                                    if let ThreadState::WaitingForMutex(data) = state.deref()
                                        && data.mutex_id == id
                                    {
                                        let saved_regs = data.saved_regs.clone();
                                        let mut state = state.upgrade();
                                        *state = ThreadState::Ready(ThreadReadyState::InSyscall(
                                            saved_regs,
                                        ));
                                        *running_thread = None;
                                        mutex.locked_by = *thread_id;
                                        log::debug!("giving lock to {thread_id:?}");
                                        break Some(Action::RunThreads);
                                    }
                                }
                                None => {
                                    break None;
                                }
                            }
                        } {
                            action
                        } else {
                            mutexes.remove(&id);
                            Action::Return
                        }
                    } else {
                        Action::Terminate
                    }
                }
                None => Action::Terminate,
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
