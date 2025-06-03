use core::arch::asm;

use x86_64::{VirtAddr, registers::rflags::RFlags};

pub struct EnterUserModeInput {
    pub rip: VirtAddr,
    // We don't use `VirtAddr` for rsp because 0x800000000000 is a valid rsp value, which is not a valid VirtAddr
    pub rsp: u64,
    pub rflags: RFlags,
}

/// # Safety
/// Does sysret.
/// Make sure that you are not letting the user space program do things you don't want it to do.
/// You must enable system call extensions first.
pub unsafe fn enter_user_mode(EnterUserModeInput { rip, rsp, rflags }: EnterUserModeInput) -> ! {
    let rip = rip.as_u64();
    let rflags = rflags.bits();
    unsafe {
        // Note that we do `sysretq` and not `sysret` because if we just do `sysret` that could be compiled into a `sysretl`, which is for 32-bit compatibility mode and can mess things up.
        asm!("\
            mov rsp, {}
            sysretq",
            in(reg) rsp,
            in("rcx") rip,
            in("r11") rflags,
            // The user space program can only "return" with a `syscall`, which will jump to the syscall handler
            options(noreturn)
        );
    }
}
