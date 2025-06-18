use core::arch::naked_asm;

use x86_64::structures::idt::InterruptStackFrame;

use crate::{
    cpu_local_data::get_local,
    interrupted_context::InterruptedContext,
    run_tasks::run_threads,
    task::{THREADS, ThreadReadyState, ThreadState},
};

#[unsafe(naked)]
pub unsafe extern "sysv64" fn raw_check_tasks_ipi_handler(_stack_frame: InterruptStackFrame) {
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

            call {check_tasks_ipi_handler}
            ",
        check_tasks_ipi_handler = sym check_tasks_ipi_handler
    )
}

extern "sysv64" fn check_tasks_ipi_handler(interrupted_context: &mut InterruptedContext) {
    {
        let threads = THREADS.read();
        if let Some(running_thread_id) = get_local().running_thread.try_lock().unwrap().take() {
            *threads
                .get(&running_thread_id)
                .unwrap()
                .state
                .try_write()
                .unwrap() =
                ThreadState::Ready(ThreadReadyState::Interrupted(interrupted_context.clone()));
        }
    }
    run_threads()
}
