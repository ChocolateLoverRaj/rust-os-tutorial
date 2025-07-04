use core::{
    ops::DerefMut,
    sync::atomic::{AtomicBool, Ordering},
};

use common::{Syscall, SyscallCreateChannel, SyscallTxSend};
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{
        CHANNELS, Channel, EVENT_ID, THREADS, ThreadReadyState, ThreadReadyStateInSyscall,
        ThreadState,
    },
};

use super::GenericSyscallHandler;

pub struct SyscallCreateChannelHandler;
impl GenericSyscallHandler for SyscallCreateChannelHandler {
    type S = SyscallCreateChannel;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let local = get_local();
            let running_thread_id = local.running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let running_thread = threads.get(&running_thread_id).unwrap();
            let event_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
            let mut channels = CHANNELS.write();
            let process_id = running_thread.process.id;
            channels.insert(
                event_id,
                Channel {
                    receiver: process_id,
                    sender: process_id,
                    pending_event: AtomicBool::new(false),
                },
            );
            event_id
        };
        helper.syscall_return(&output)
    }
}

pub struct SyscallTxSendHandler;
impl GenericSyscallHandler for SyscallTxSendHandler {
    type S = SyscallTxSend;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return,
            RunThreads,
        }
        let action = {
            let local = get_local();
            let mut running_thread_id_guard = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread_id_guard.unwrap();
            let threads = THREADS.read();
            let running_thread = threads.get(&running_thread_id).unwrap();
            let channel_id = *helper.input();
            let channels = CHANNELS.read();
            if let Some(channel) = channels.get(&channel_id)
                && channel.sender == running_thread.process.id
            {
                let became_pending = !channel.pending_event.swap(true, Ordering::Relaxed);
                if became_pending {
                    if threads
                        .iter()
                        .filter(|(_thread_id, thread)| thread.process.id == channel.receiver)
                        .any(|(_thread_id, thread)| {
                            let mut thread_state = thread.state.write();
                            if let ThreadState::WaitingForEvents(state) = thread_state.deref_mut() {
                                if let Some(happened) = state.events.get_mut(&channel_id) {
                                    *happened = true;
                                    *running_thread_id_guard = None;
                                    *running_thread.state.write() = ThreadState::Ready(
                                        ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                                            output: <Self::S as Syscall>::encode_output(&()),
                                            saved_regs: helper.saved_regs().clone(),
                                        }),
                                    );
                                    let mut local_apic =
                                        local.local_apic.get().unwrap().try_lock().unwrap();
                                    unsafe {
                                        local_apic.send_ipi_all(
                                            InterruptVector::CheckTasks.into(),
                                            IpiAllShorthand::AllExcludingSelf,
                                        )
                                    };
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                    {
                        Action::RunThreads
                    } else {
                        Action::Return
                    }
                } else {
                    Action::Return
                }
            } else {
                todo!()
            }
        };
        match action {
            Action::Return => helper.syscall_return(&()),
            Action::RunThreads => run_threads(),
        }
    }
}
