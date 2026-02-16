#![no_std]
#![no_main]
#![feature(stdarch_arm_hints, stdarch_arm_neon_intrinsics)]

use core::arch::arm::__wfe;
use core::arch::naked_asm;
use core::panic::PanicInfo;

unsafe extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    let _ = panic_info;
    loop {}
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(_r0: usize, machine_id: usize, atags_ptr: usize) -> ! {
    loop {
        unsafe { __wfe() };
    }
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Set the stack pointer to the stack space we reserved in the linker script
        ldr sp, =__stack_top

        // Zero the BSS. Zero it by 4 * usize at a time instead of one byte or one usize at a time
        ldr r4, =__bss_start
        ldr r9, =__bss_end
        // Set r5-r8 to 0
        mov r5, #0
        mov r6, #0
        mov r7, #0
        mov r8, #0
        // Start by checking for the end condition
        b while

        do:
            // This stores the values of registers r5-r8 at the value of r4, incrementing r4 by
            // size_of::<usize> as it stores each register
            stmia r4!, {{r5-r8}}

        while:
            // If r4 < r9, jump to `do`
            cmp r4, r9
            blo do
            // Else, continue executing the instructions below
            // Call kernel_main
            blx {kernel_main}
        ",
        kernel_main = sym kernel_main
    )
}
