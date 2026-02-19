use core::arch::{asm, naked_asm};

use aarch64_cpu::registers::{
    CPTR_EL2, CurrentEL, DAIF, ELR_EL2, ESR_EL2, HCR_EL2, MIDR_EL1, ReadWriteable, Readable,
    SP_EL2, SPSel, VBAR_EL2, Writeable,
};
use arbitrary_int::u12;
use log::info;

use crate::{
    __interrupt_handler_stack_top, RPI_3_PART_NO, RPI_4_PART_NO, halt_loop, init_common, logger,
};

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Entry point for the kernel. Registers:
        // x0 -> 32 bit pointer to DTB in memory (primary core only) / 0 (secondary cores)
        // x2 -> 0
        // x2 -> 0
        // x3 -> 0
        // x4 -> 32 bit kernel entry point, _start location
        // Set the stack pointer to the stack space we reserved in the linker script
        ldr x5, =__stack_top
        mov sp, x5

        // clear bss
        ldr x5, =__bss_start
        ldr w6, =__bss_size
        1:
            cbz     w6, 2f
            str     xzr, [x5], #8
            sub     w6, w6, #1
            cbnz    w6, 1b
        2:
            bl      {kernel_main}
        ",
        kernel_main = sym kernel_main_64
    )
}

fn get_mmio_base() -> usize {
    let midr = MIDR_EL1.get();
    let part_no = MIDR_EL1::PartNum.read(midr) as u16;
    match part_no {
        RPI_3_PART_NO => 0x3F000000,
        RPI_4_PART_NO => 0xFE000000,
        _ => panic!("unknown part number"),
    }
}

fn kernel_main_64(dtb_ptr: u32, _x1: usize, _x2: usize, _x3: usize) -> ! {
    use aarch64_cpu::registers::{MIDR_EL1, Readable};

    logger::init();

    info!(
        "Hello from Rust kernel booted on 64 bit ARM. This is likely booted on a Raspberry Pi 3 or 4. Device Tree pointer: {dtb_ptr:#X}."
    );

    // TODO: Use device tree
    let midr = MIDR_EL1.get();
    init_common(u12::from_u64(MIDR_EL1::PartNum.read(midr)));

    let current_el = CurrentEL::EL.read(CurrentEL.get());
    let daif = DAIF.get();
    info!("daif: {daif:#X}");

    info!("current el: {current_el}");
    // CPTR_EL2.set(0);
    let vector_table_addr = vector_table as *const () as usize;
    VBAR_EL2.set(vector_table_addr as u64);
    info!("set vector table addr: {vector_table_addr:#X}.");

    DAIF.set(0);
    let mut hcr_el2 = HCR_EL2.get();
    // hcr_el2 |= 1 << 3;
    hcr_el2 |= 1 << 4;
    HCR_EL2.set(hcr_el2);

    // let interrupt_handler_stack_top =
    //     core::ptr::from_ref(unsafe { &__interrupt_handler_stack_top }).addr() as u64;
    // info!("setting interrupt handler stack top: {interrupt_handler_stack_top:#X}");
    // let sp_sel = SPSel.get();
    // info!("SPSel: {sp_sel:#b}");
    // let sp_el2 = SP_EL2.get();
    // info!("SP_EL2: {sp_el2:#X}");
    // SP_EL2.set(interrupt_handler_stack_top);
    unsafe {
        asm!(
            "
            dsb ish
            isb
            "
        )
    };
    // DAIF.set(0);

    // info!("testing HVC exception");
    // unsafe { asm!("hvc #0") };
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

    // let daif = DAIF.get();
    // info!("DAIF: {daif:#b}");

    halt_loop()
}

#[unsafe(naked)]
extern "C" fn vector_table() {
    naked_asm!(
        "
        // The table must be aligned
        .balign 2048
        // SP_EL0
        .balign 0x80
            mov x0, #0
            b {interrupt_handler}
        .balign 0x80
            mov x0, #1
            b {interrupt_handler}
        .balign 0x80
            mov x0, #2
            b {interrupt_handler}
        .balign 0x80
            mov x2, #3
            b {interrupt_handler}
        // SP_ELx
        .balign 0x80
            mov x0, #4
            b {interrupt_handler}
        .balign 0x80
            mov x0, #5
            b {interrupt_handler}
        .balign 0x80
            mov x0, #6
            b {interrupt_handler}
        .balign 0x80
            mov x2, #7
            b {interrupt_handler}
        // From lower EL
        .balign 0x80
            mov x0, #8
            b {interrupt_handler}
        .balign 0x80
            mov x0, #9
            b {interrupt_handler}
        .balign 0x80
            mov x0, #10
            b {interrupt_handler}
        .balign 0x80
            mov x2, #11
            b {interrupt_handler}
        // From lower EL
        .balign 0x80
            mov x0, #12
            b {interrupt_handler}
        .balign 0x80
            mov x0, #13
            b {interrupt_handler}
        .balign 0x80
            mov x0, #14
            b {interrupt_handler}
        .balign 0x80
            mov x0, #15
            b {interrupt_handler}
        ",
        interrupt_handler = sym interrupt_handler,
    )
}

unsafe extern "C" fn interrupt_handler(source: usize) -> ! {
    let esr = ESR_EL2.get();
    let elr = ELR_EL2.get();
    panic!("interrupt / exception. source: {source}. esr: {esr:#X}. elr: {elr:#X}.");
}
