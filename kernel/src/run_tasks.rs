use core::ops::Deref;

use x86_64::{
    instructions::interrupts,
    registers::{control::Cr3, rflags::RFlags},
};

use crate::{
    cpu_local_data::get_local,
    enter_user_mode::{EnterUserModeInput, enter_user_mode},
    hlt_loop::hlt_loop,
    memory::MEMORY,
    task::{THREAD_PRIORITIES, THREADS, ThreadReadyState, ThreadState, ThreadWaitingState},
};

pub fn run_threads() -> ! {
    #[derive(Debug)]
    enum Action {
        Start(ThreadReadyState),
        ReturnFromWait(ThreadWaitingState),
        DoNothing,
    }
    let action = {
        let thread_priorities = THREAD_PRIORITIES.read();
        // log::debug!("Thread priorities: {thread_priorities:?}");
        let threads = THREADS.read();
        let mut thread_priorities = thread_priorities.iter();
        let local = get_local();
        let r = { local.running_thread.lock() }.clone();

        assert!(r.is_none());
        loop {
            if let Some(thread_id) = thread_priorities.next() {
                let thread = threads.get(thread_id).unwrap();
                let mut thread_state = thread.state.write();
                // log::debug!("Tjread: {thread_id:?} {thread_state:?}");
                match thread_state.deref() {
                    ThreadState::Ready(ready_state) => {
                        let action = Action::Start(ready_state.clone());
                        *local.running_thread.try_lock().unwrap() = Some(*thread_id);
                        *thread_state = ThreadState::Running(local.cpu.into());
                        unsafe {
                            Cr3::write(
                                thread.process.cr3,
                                MEMORY.get().unwrap().new_kernel_cr3_flags,
                            )
                        };
                        break action;
                    }
                    ThreadState::WaitingForEvents(state) => {
                        if state.events.values().any(|happened| *happened) {
                            let action = Action::ReturnFromWait(state.clone());
                            let local = get_local();
                            *local.running_thread.try_lock().unwrap() = Some(*thread_id);
                            *thread_state = ThreadState::Running(local.cpu.into());
                            unsafe {
                                Cr3::write(
                                    thread.process.cr3,
                                    MEMORY.get().unwrap().new_kernel_cr3_flags,
                                )
                            };
                            break action;
                        } else {
                            // log::debug!("{thread_id:?} is waiting for events");
                        }
                    }
                    ThreadState::Running(_) => {
                        // log::debug!("{thread_id:?} is already running");
                    }
                    ThreadState::WaitingForMutex(_data) => {
                        // TODO: Run the thread that's holding the mutex lock (priority inheritance / priority boosting)
                        // log::debug!("{thread_id:?} is waiting for mutex");
                    }
                }
            } else {
                break Action::DoNothing;
            }
        }
    };
    // log::debug!("Action: {:?}", action);
    match action {
        Action::Start(ThreadReadyState::ReadyToStart(start_data)) => {
            let input = EnterUserModeInput {
                rip: start_data.rip,
                rsp: start_data.rsp,
                rflags: RFlags::INTERRUPT_FLAG,
            };
            unsafe { enter_user_mode(input) }
        }
        Action::Start(ThreadReadyState::Interrupted(interrupted_context)) => unsafe {
            interrupted_context.restore()
        },
        Action::Start(ThreadReadyState::InSyscall(data)) => unsafe { data.sysretq() },
        Action::ReturnFromWait(state) => unsafe { state.sysretq() },
        Action::DoNothing => {
            // log::debug!("No threads to run. Doing nothing.");
            interrupts::enable();
            hlt_loop()
        }
    }
}
