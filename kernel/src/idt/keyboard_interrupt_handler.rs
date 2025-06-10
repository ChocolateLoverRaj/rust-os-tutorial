use core::{
    arch::{asm, naked_asm},
    mem,
};

use common::{Syscall, SyscallWaitUntilEvent};
use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

use crate::{
    cpu_local_data::get_local,
    syscall_saved_regs::SyscallSavedRegs,
    task::{TASK, TaskState},
};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct FullContext {
    pub rbp: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl FullContext {
    unsafe fn restore(&self) -> ! {
        unsafe {
            asm!("\
                mov rsp, {}
                pop rbp
                pop rax
                pop rbx
                pop rcx
                pop rdx
                pop rsi
                pop rdi
                pop r8
                pop r9
                pop r10
                pop r11
                pop r12
                pop r13
                pop r14
                pop r15
                iretq
                ",
                in(reg) self
            );
        }
        unreachable!()
    }
}

/// <https://wiki.osdev.org/Interrupt_Service_Routines#x86-64>
/// <https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI>
/// All we do is push the registers on the stack that we want to save and then call the Rust function
#[unsafe(naked)]
pub unsafe extern "sysv64" fn raw_keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
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

            call {keyboard_interrupt_handler}
            ",
        keyboard_interrupt_handler = sym keyboard_interrupt_handler
    )
}

pub extern "sysv64" fn keyboard_interrupt_handler(full_context: &mut FullContext) -> ! {
    struct RestoreSyscallData {
        saved_regs: SyscallSavedRegs,
        output: [u64; 7],
    }
    enum Action {
        RestoreInterrupted(FullContext),
        RestoreSyscall(RestoreSyscallData),
    }
    let action = {
        let mut port = Port::<u8>::new(0x60);
        let data = unsafe { port.read() };
        log::debug!("Keyboard interrupt received: {data}");
        let local = get_local();

        let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
        unsafe { local_apic.end_of_interrupt() };

        let mut task = TASK.try_lock().unwrap();
        if let Some(task) = task.as_mut()
            && let Some(keyboard_queue) = task.keyboard.as_mut()
        {
            keyboard_queue.queue.force_push(data);
            match mem::replace(&mut task.state, TaskState::Running) {
                TaskState::Running => {
                    keyboard_queue.pending_event = true;
                    Action::RestoreInterrupted(full_context.clone())
                }
                TaskState::Waiting(waiting_state) => Action::RestoreSyscall(RestoreSyscallData {
                    saved_regs: waiting_state.saved_regs,
                    output: SyscallWaitUntilEvent::encode_output(&1),
                }),
            }
        } else {
            Action::RestoreInterrupted(full_context.clone())
        }
    };
    match action {
        Action::RestoreInterrupted(full_context) => unsafe { full_context.restore() },
        Action::RestoreSyscall(RestoreSyscallData { saved_regs, output }) => unsafe {
            saved_regs.sysretq(output)
        },
    }
}
