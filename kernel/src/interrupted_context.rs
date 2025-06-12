use core::arch::asm;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InterruptedContext {
    rbp: u64,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

impl InterruptedContext {
    /// # Safety
    /// Completely changes registers. Make sure that you don't restore invalid context and don't leak information to user mode.
    pub unsafe fn restore(&self) -> ! {
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
