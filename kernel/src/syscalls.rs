use core::arch::naked_asm;

use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::{LStar, SFMask},
        rflags::RFlags,
    },
};

#[unsafe(naked)]
unsafe extern "sysv64" fn syscall_handler() {
    naked_asm!("ud2")
}

pub fn init() {
    // Enable syscall in IA32_EFER
    // https://shell-storm.org/x86doc/SYSCALL.html
    // https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
    unsafe {
        Efer::update(|flags| {
            *flags = flags.union(EferFlags::SYSTEM_CALL_EXTENSIONS);
        })
    };

    // clear Interrupt flag on syscall with AMD's MSR_FMASK register
    // This makes it so that interrupts are disabled during the syscall handler
    SFMask::write(RFlags::INTERRUPT_FLAG);

    // write handler address to AMD's MSR_LSTAR register
    LStar::write(VirtAddr::from_ptr(syscall_handler as *const ()));
}
