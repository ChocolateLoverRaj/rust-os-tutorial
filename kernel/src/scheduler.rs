use core::ops::Deref;

use alloc::sync::Arc;
use ez_paging::ManagedL4PageTable;
use spin::Once;
use spinning_top::lock_api;
use x86_64::{instructions::interrupts, registers::rflags::RFlags};

use crate::{
    EnterUserModeInput, cpu_local_data::get_local, enter_user_mode, hlt_loop::hlt_loop,
    memory::MEMORY, syscall_handler,
};

// pub struct Container {
//     pub id: u32,
//     pub inside: Arc<Task>,
//     pub below: Option<Adrc<Task>>,
// }

#[derive(Debug, Clone, Copy)]
pub struct StartData {
    pub rip: u64,
    pub rsp: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ThreadState {
    ReadyToStart(StartData),
    Ended,
}

pub struct Thread {
    // pub id: u32,
    pub state: Arc<spinning_top::Spinlock<ThreadState>>,
    pub address_space: ManagedL4PageTable,
    // pub below: Option<Arc<Task>>,
}

// pub enum Task {
//     Container(Arc<Container>),
//     Thread(Arc<Thread>),
// }

static ROOT_TASK: Once<Thread> = Once::new();

/// This function must be called on all CPUs
pub fn init() {
    syscall_handler::init();
}

pub fn init_root_task(task: Thread) {
    ROOT_TASK.call_once(|| task);
}

pub fn run_tasks() -> ! {
    if let Some(thread) = ROOT_TASK.get() {
        if let Some(state) = lock_api::Mutex::try_lock_arc(&thread.state) {
            match *state.deref() {
                ThreadState::ReadyToStart(start_data) => {
                    // Switch to the user address space
                    // Safety: we can still reference kernel memory
                    unsafe {
                        thread
                            .address_space
                            .switch_to(MEMORY.get().unwrap().new_kernel_cr3_flags)
                    };
                    *get_local().running_thread.try_lock().unwrap() = Some(state);
                    unsafe {
                        enter_user_mode(EnterUserModeInput {
                            rip: start_data.rip,
                            rsp: start_data.rsp,
                            rflags: RFlags::empty(),
                        })
                    }
                }
                ThreadState::Ended => {
                    // TODO: Check below
                    no_tasks_to_run()
                }
            }
        } else {
            // TODO: Check below
            no_tasks_to_run()
        }
    } else {
        no_tasks_to_run()
    }
}

fn no_tasks_to_run() -> ! {
    log::debug!("No tasks to run");
    interrupts::enable();
    hlt_loop()
}
