#![no_std]
#![no_main]
mod logger;
mod sbi;

use core::{
    arch::{asm, naked_asm},
    panic::PanicInfo,
    ptr::write_bytes,
};

use log::error;
use riscv::{
    self as _, read_csr,
    register::{
        scause, sepc, stval,
        stvec::{self, Stvec, TrapMode},
    },
};

use crate::sbi::shutdown;

unsafe extern "C" {
    static __bss: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

extern "C" fn kernel_main(hart_id: usize, ftd_ptr: usize) {
    unsafe { write_bytes(__bss as *mut u8, 0, __bss_end - __bss) };
    unsafe {
        logger::init();
    }

    log::info!("Hello from Rust!. HART ID: {hart_id}. Device Tree pointer: {ftd_ptr:#X}.");

    unsafe {
        stvec::write(Stvec::new(
            kernel_entry as *const () as usize,
            TrapMode::Direct,
        ))
    };
    unsafe { asm!("unimp") };

    shutdown()
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn boot() {
    naked_asm!(
        "
            la sp, {stack_top}
            j {kernel_main}
        ",
        stack_top = sym __stack_top,
        kernel_main = sym kernel_main
    )
}

#[repr(C, packed)]
pub struct TrapFrame {
    pub ra: u32,
    pub gp: u32,
    pub tp: u32,
    pub t0: u32,
    pub t1: u32,
    pub t2: u32,
    pub t3: u32,
    pub t4: u32,
    pub t5: u32,
    pub t6: u32,
    pub a0: u32,
    pub a1: u32,
    pub a2: u32,
    pub a3: u32,
    pub a4: u32,
    pub a5: u32,
    pub a6: u32,
    pub a7: u32,
    pub s0: u32,
    pub s1: u32,
    pub s2: u32,
    pub s3: u32,
    pub s4: u32,
    pub s5: u32,
    pub s6: u32,
    pub s7: u32,
    pub s8: u32,
    pub s9: u32,
    pub s10: u32,
    pub s11: u32,
    pub sp: u32,
}

#[unsafe(naked)]
extern "C" fn kernel_entry() {
    naked_asm!(
        "
            .align 4
            csrw sscratch, sp
            addi sp, sp, -4 * 31

            sw ra,  4 * 0(sp)
            sw gp,  4 * 1(sp)
            sw tp,  4 * 2(sp)
            sw t0,  4 * 3(sp)
            sw t1,  4 * 4(sp)
            sw t2,  4 * 5(sp)
            sw t3,  4 * 6(sp)
            sw t4,  4 * 7(sp)
            sw t5,  4 * 8(sp)
            sw t6,  4 * 9(sp)
            sw a0,  4 * 10(sp)
            sw a1,  4 * 11(sp)
            sw a2,  4 * 12(sp)
            sw a3,  4 * 13(sp)
            sw a4,  4 * 14(sp)
            sw a5,  4 * 15(sp)
            sw a6,  4 * 16(sp)
            sw a7,  4 * 17(sp)
            sw s0,  4 * 18(sp)
            sw s1,  4 * 19(sp)
            sw s2,  4 * 20(sp)
            sw s3,  4 * 21(sp)
            sw s4,  4 * 22(sp)
            sw s5,  4 * 23(sp)
            sw s6,  4 * 24(sp)
            sw s7,  4 * 25(sp)
            sw s8,  4 * 26(sp)
            sw s9,  4 * 27(sp)
            sw s10, 4 * 28(sp)
            sw s11, 4 * 29(sp)

            csrr a0, sscratch
            sw a0, 4 * 30(sp)

            mv a0, sp
            call {handle_trap}

            lw ra,  4 * 0(sp)
            lw gp,  4 * 1(sp)
            lw tp,  4 * 2(sp)
            lw t0,  4 * 3(sp)
            lw t1,  4 * 4(sp)
            lw t2,  4 * 5(sp)
            lw t3,  4 * 6(sp)
            lw t4,  4 * 7(sp)
            lw t5,  4 * 8(sp)
            lw t6,  4 * 9(sp)
            lw a0,  4 * 10(sp)
            lw a1,  4 * 11(sp)
            lw a2,  4 * 12(sp)
            lw a3,  4 * 13(sp)
            lw a4,  4 * 14(sp)
            lw a5,  4 * 15(sp)
            lw a6,  4 * 16(sp)
            lw a7,  4 * 17(sp)
            lw s0,  4 * 18(sp)
            lw s1,  4 * 19(sp)
            lw s2,  4 * 20(sp)
            lw s3,  4 * 21(sp)
            lw s4,  4 * 22(sp)
            lw s5,  4 * 23(sp)
            lw s6,  4 * 24(sp)
            lw s7,  4 * 25(sp)
            lw s8,  4 * 26(sp)
            lw s9,  4 * 27(sp)
            lw s10, 4 * 28(sp)
            lw s11, 4 * 29(sp)

            lw sp,  4 * 30(sp)
            sret
        ",
        handle_trap = sym handle_trap
    );
}

extern "C" fn handle_trap(trap_frame: &TrapFrame) {
    let scause = scause::read();
    let stval = stval::read();
    let user_pc = sepc::read();
    panic!("unexpected trap. scause={scause:#X?}. stval={stval:#X}. sepc={user_pc:#X}.");
}
