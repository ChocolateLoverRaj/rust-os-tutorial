use core::arch::naked_asm;

use x86_64::structures::idt::InterruptStackFrame;

use crate::{
    interrupted_context::InterruptedContext, ps2_interrupt_handler::ps2_interrupt_handler,
    task::EventStreamSource,
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
    // Safety: this is from a PS/2 mouse interrupt handler
    unsafe { ps2_interrupt_handler(interrupted_context, EventStreamSource::Ps2Mouse) }
}
