use core::arch::asm;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SyscallSavedRegs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    r11: u64,
    rcx: u64,
    rsp: u64,
}

impl SyscallSavedRegs {
    /// # Safety
    /// This will enter user mode with the registers set according to the saved registers and the output
    pub unsafe fn sysretq(&self, output: [u64; 7]) -> ! {
        unsafe {
            asm!(
                "
                mov rsp, {}
                pop r15
                pop r14
                pop r13
                pop r12
                pop rbx
                pop rbp
                pop r11
                pop rcx
                pop rsp
                sysretq
            ",
                in(reg) self,
                in("rdi") output[0],
                in("rsi") output[1],
                in("rdx") output[2],
                in("r10") output[3],
                in("r8") output[4],
                in("r9") output[5],
                in("rax") output[6],
                options(noreturn)
            );
        }
    }
}
