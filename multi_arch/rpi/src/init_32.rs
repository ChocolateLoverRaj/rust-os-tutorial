use core::arch::{arm::__wfi, naked_asm};

use aarch32_cpu::{asm::irq_enable, register::Midr};
use arbitrary_int::u12;
use log::info;

use crate::{RPI_0_1_PART_NO, RPI_2_PART_NO, halt_loop::halt_loop, init_common, logger};

#[cfg(all(target_arch = "arm", not(target_feature = "v7")))]
#[unsafe(link_section = ".text._start")]
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
        kernel_main = sym kernel_main_32
    )
}

#[cfg(all(target_arch = "arm", target_feature = "v7"))]
#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Shut off extra cores
        mrc p15, 0, r5, c0, c0, 5
        and r5, r5, #3
        cmp r5, #0
	    bne halt

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

        halt:
            wfe
            b halt
        ",
        kernel_main = sym kernel_main_32
    )
}

fn get_mmio_base() -> usize {
    match Midr::read().part_no() {
        i if i == u12::new(RPI_0_1_PART_NO) => 0x20000000,
        i if i == u12::new(RPI_2_PART_NO) => 0x3F000000,
        _ => panic!("unknown part number"),
    }
}

extern "C" fn kernel_main_32(_r0: usize, _machine_id: usize, _atags_ptr: usize) -> ! {
    logger::init();

    info!(
        "Hello from Rust kernel booted on 32 bit ARM. This is likely booted on a Raspberry Pi Zero, 1, or 2."
    );
    let midr = Midr::read();
    let vectors_addr = vectors as *const () as usize;
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {0}, c12, c0, 0", // Write to VBAR
            in(reg) vectors_addr
        );
    }
    init_common(midr.part_no());

    info!("wrote to VBAR {vectors_addr:#X}");

    // Set the stack pointer of the UND handler
    unsafe {
        core::arch::asm!(
            // 1. Change the CPSR mode bits to 0x1B (Undefined Mode)
            // We use 0xDB to also keep interrupts disabled (I and F bits)
            "msr cpsr_c, #0xdb",
            // 2. Now 'sp' refers to the banked SP_und register
            "ldr sp, =__interrupt_handler_stack_top",
            // 3. Switch back to Supervisor Mode (0xD3)
            "msr cpsr_c, #0xd3",
        );
    }

    // Set the stack pointer of the IRQ handler
    unsafe {
        core::arch::asm!(
            "mrs r0, cpsr",      // Save current (SVC) mode
            "msr cpsr_c, #0xd2", // Switch to IRQ Mode (0x12 | 0xC0)
            "ldr sp, =__interrupt_handler_stack_top",   // Set a 4KB-aligned stack pointer
            "msr cpsr_c, r0",    // Switch back to SVC Mode
            out("r0") _,
            options(nomem, nostack)
        );
    }

    unsafe { irq_enable() };

    let mmio_base = get_mmio_base();
    let irq_enable_1 = (mmio_base + 0xB210) as *mut u32;

    unsafe {
        // Enable IRQ 1 (System Timer Compare 1)
        core::ptr::write_volatile(irq_enable_1, 1 << 1);
    }

    let timer_cs = (mmio_base + 0x3000) as *mut u32;
    unsafe {
        // Write a 1 to bit 1 to CLEAR any pending interrupt for Compare 1
        core::ptr::write_volatile(timer_cs, 1 << 1);
    }

    let timer_clo: *mut u32 = (mmio_base + 0x3004) as *mut u32; // Lower 32 bits of counter
    let timer_c1: *mut u32 = (mmio_base + 0x3010) as *mut u32; // Compare register 1

    unsafe {
        // 1. Read current time
        let current_val = core::ptr::read_volatile(timer_clo);

        // 2. Set match for 1 second from now (System Timer runs at 1MHz)
        let match_val = current_val.wrapping_add(1_000_000);

        // 3. Write to Compare 1
        core::ptr::write_volatile(timer_c1, match_val);
    }

    info!("enabled the timer");

    loop {
        unsafe { __wfi() };
        info!("after wfi");
    }
}

#[unsafe(naked)]
extern "C" fn vectors() {
    naked_asm!(
        "
        .balign 32
        ldr pc, ={reset_handler}
        ldr pc, ={und_handler}
        ldr pc, ={svc_handler}
        ldr pc, ={prefetch_abort_handler}
        ldr pc, ={data_abort_handler}
        nop // reserved
        ldr pc, ={irq_handler}
        ldr pc, ={fiq_handler}
        ",
        reset_handler = sym reset_handler,
        und_handler = sym und_handler,
        svc_handler = sym svc_handler,
        prefetch_abort_handler = sym prefetch_abort_handler,
        data_abort_handler = sym data_abort_handler,
        irq_handler = sym irq_handler,
        fiq_handler = sym fiq_handler
    )
}

extern "C" fn reset_handler() {
    info!("reset handler");
    halt_loop()
}

extern "C" fn und_handler() {
    info!("und handler");
    halt_loop()
}

extern "C" fn svc_handler() {
    info!("svc handler");
    halt_loop()
}
extern "C" fn prefetch_abort_handler() {
    info!("prefetch abort handler");
    halt_loop()
}
extern "C" fn data_abort_handler() {
    info!("data abort handler");
    halt_loop()
}
extern "C" fn irq_handler() {
    info!("irq handler");
    let mmio_base = get_mmio_base();
    let timer_cs = (mmio_base + 0x3000) as *mut u32;
    unsafe {
        // Write a 1 to bit 1 to CLEAR any pending interrupt for Compare 1
        core::ptr::write_volatile(timer_cs, 1 << 1);
    }
}
extern "C" fn fiq_handler() {
    info!("fiq handler");
    halt_loop()
}
