use core::arch::naked_asm;

use common::{Syscall, SyscallWaitUntilEvent};
use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

use crate::{
    cpu_local_data::get_local,
    interrupted_context::InterruptedContext,
    syscall_handlers::MOUSE_EVENT_ID,
    syscall_saved_regs::SyscallSavedRegs,
    task::{TASK, TaskState},
};

#[unsafe(naked)]
pub unsafe extern "sysv64" fn raw_mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    naked_asm!(
        "
            push r15
            push r14
            push r13
            push r12
            push r11
            push r10
            push r9
            push r8
            push rdi
            push rsi
            push rdx
            push rcx
            push rbx
            push rax
            push rbp

            mov rdi, rsp   // first arg of context switch is the context which is all the registers saved above

            call {mouse_interrupt_handler}
            ",
        mouse_interrupt_handler = sym mouse_interrupt_handler
    )
}

pub unsafe extern "sysv64" fn mouse_interrupt_handler(
    interrupted_context: &mut InterruptedContext,
) {
    struct RestoreSyscallData {
        saved_regs: SyscallSavedRegs,
        output: [u64; 7],
    }
    enum Action {
        RestoreInterrupted,
        RestoreSyscall(RestoreSyscallData),
    }
    let action = {
        let mut port = Port::<u8>::new(0x60);
        let data = unsafe { port.read() };
        let local = get_local();
        let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
        unsafe { local_apic.end_of_interrupt() };

        let mut task = TASK.try_lock().unwrap();
        if let Some(task) = task.as_mut()
            && let Some(mouse) = task.mouse.as_mut()
        {
            mouse.queue.force_push(data);
            match &task.state {
                TaskState::Running => {
                    mouse.pending_event = true;
                    Action::RestoreInterrupted
                }
                TaskState::Waiting(waiting_state) => {
                    let events =
                        unsafe { waiting_state.events_slice.try_to_slice_mut::<u64>() }.unwrap();
                    if events.contains(&MOUSE_EVENT_ID) {
                        events[0] = MOUSE_EVENT_ID;
                        let action = Action::RestoreSyscall(RestoreSyscallData {
                            saved_regs: waiting_state.saved_regs.clone(),
                            output: SyscallWaitUntilEvent::encode_output(&1),
                        });
                        task.state = TaskState::Running;
                        action
                    } else {
                        Action::RestoreInterrupted
                    }
                }
            }
        } else {
            Action::RestoreInterrupted
        }
    };
    match action {
        Action::RestoreInterrupted => unsafe { interrupted_context.restore() },
        Action::RestoreSyscall(RestoreSyscallData { saved_regs, output }) => unsafe {
            saved_regs.sysretq(output)
        },
    }
}
