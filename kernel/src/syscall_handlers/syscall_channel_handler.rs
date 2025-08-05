use core::{ops::DerefMut, sync::atomic::Ordering};

use alloc::sync::Arc;
use common::{Syscall, SyscallCreateChannel, SyscallTxSend};
use x2apic::lapic::IpiAllShorthand;

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId, CapabilityType},
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{THREADS, ThreadReadyState, ThreadReadyStateInSyscall, ThreadState},
};

use super::GenericSyscallHandler;

pub struct SyscallCreateChannelHandler;
impl GenericSyscallHandler for SyscallCreateChannelHandler {
    type S = SyscallCreateChannel;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let capability_id = CapabilityId::new_unique();
            let mut capabilities = CAPABILITIES.write();

            let local = get_local();
            let running_thread_id = local.running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let running_thread = threads.get(&running_thread_id).unwrap();
            capabilities.insert(
                capability_id.into(),
                Capability {
                    _type: CapabilityType::Channel(Default::default()),
                    process_id: running_thread.process.id.into(),
                },
            );
            capability_id.into()
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
            let capability_id = *helper.input();
            let capabilities = CAPABILITIES.read();
            if let Some(capability) = capabilities.get(&capability_id)
                && capability.process_id == running_thread.process.id.into()
                && let CapabilityType::Channel(happened) = &capability._type
            {
                let became_pending = !happened.swap(true, Ordering::Relaxed);
                if became_pending {
                    if threads.iter().any(|(_thread_id, thread)| {
                        let mut thread_state = thread.state.write();
                        if let ThreadState::WaitingForEvents(state) = thread_state.deref_mut() {
                            if let Some(happened) =
                                state.events.iter_mut().find_map(|(capability, h)| {
                                    match &capabilities.get(capability).unwrap()._type {
                                        CapabilityType::Channel(arc) => {
                                            if Arc::ptr_eq(arc, happened) {
                                                Some(h)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                })
                            {
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
                    }) {
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
