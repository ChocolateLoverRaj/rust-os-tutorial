use core::{arch::naked_asm, ops::Deref, sync::atomic::Ordering};

use x2apic::lapic::IpiAllShorthand;
use x86_64::structures::idt::InterruptStackFrame;

use crate::{
    cpu_local_data::get_local,
    cpus_count,
    interrupt_vector::InterruptVector,
    interrupted_context::InterruptedContext,
    run_tasks::run_threads,
    task::{THREADS, ThreadReadyState, ThreadState},
};

#[unsafe(naked)]
pub unsafe extern "sysv64" fn raw_flush_tlb_ipi_handler(_stack_frame: InterruptStackFrame) {
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

            call {handler}
            ",
        handler = sym flush_tlb_ipi_handler
    )
}

extern "sysv64" fn flush_tlb_ipi_handler(interrupted_context: &mut InterruptedContext) {
    {
        let threads = THREADS.read();
        let local = get_local();
        if let Some(running_thread_id) = local.running_thread.try_lock().unwrap().take() {
            *threads.get(&running_thread_id).unwrap().state.write() =
                ThreadState::Ready(ThreadReadyState::Interrupted(interrupted_context.clone()));
        }
        let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
        for thread in threads.values() {
            let lock = thread.state.upgradeable_read();
            if let ThreadState::FlushingTlb(state) = lock.deref() {
                let flushed_count = state.flushed_count.fetch_add(1, Ordering::Relaxed) + 1;
                if flushed_count == cpus_count() {
                    *lock.upgrade() =
                        ThreadState::Ready(ThreadReadyState::InSyscall(state.state.clone()));
                    unsafe {
                        local_apic.send_ipi_all(
                            InterruptVector::CheckTasks.into(),
                            IpiAllShorthand::AllExcludingSelf,
                        )
                    };
                }
            };
        }
        unsafe {
            local_apic.end_of_interrupt();
        }
    }
    run_threads()
}
