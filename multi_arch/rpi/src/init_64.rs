use core::{
    arch::{asm, naked_asm},
    ptr::NonNull,
};

use aarch64_cpu::{
    asm::wfi,
    registers::{
        CurrentEL, DAIF, ELR_EL2, ESR_EL2, HCR_EL2, MIDR_EL1, Readable, VBAR_EL2, Writeable,
    },
};
use arbitrary_int::{u2, u6, u12};
use ez_mailbox::{
    interrupts::{Interrupts, InterruptsRef, InterruptsVolatileFieldAccess},
    timer::{Timer, TimerRef},
    volatile::VolatileRef,
};
use log::info;
use spin::{Mutex, Once};

use crate::{RPI_3_PART_NO, RPI_4_PART_NO, halt_loop, init_common, logger};

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

static TIMER: Once<Mutex<TimerRef<'static>>> = Once::new();
static INTERRUPTS: Once<Mutex<InterruptsRef<'static>>> = Once::new();

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

    let mut interrupts = InterruptsRef({
        let pointer = NonNull::new((mmio_base + Interrupts::ADDRESS) as *mut Interrupts).unwrap();
        unsafe { VolatileRef::new(pointer) }
    });
    interrupts.enable_irq(u6::new(1));
    interrupts.disable_irq(u6::new(1));
    interrupts.enable_irq(u6::new(1));

    let pointer = NonNull::new((mmio_base + Timer::ADDRESS) as *mut Timer).unwrap();
    let mut timer = TimerRef(unsafe { VolatileRef::new(pointer) });
    timer.clear_interrupt(u2::new(1));
    let current_val = timer.counter_lo();
    timer.write_compare_value(u2::new(1), current_val.wrapping_add(1_000_000));

    info!("enabled the timer");

    TIMER.call_once(|| Mutex::new(timer));
    INTERRUPTS.call_once(|| Mutex::new(interrupts));

    DAIF.set(0);
    loop {
        wfi();
        info!("after wfi");
    }
}

#[unsafe(naked)]
extern "C" fn vector_table() {
    naked_asm!(
        "
        .macro SAVE_CONTEXT
            // Subtract 272 bytes from SP (32 registers * 8 bytes + 16 bytes for padding/alignment)
            sub     sp, sp, #272

            // Save x0 through x29 in pairs
            stp     x0,  x1,  [sp, #16 * 0]
            stp     x2,  x3,  [sp, #16 * 1]
            stp     x4,  x5,  [sp, #16 * 2]
            stp     x6,  x7,  [sp, #16 * 3]
            stp     x8,  x9,  [sp, #16 * 4]
            stp     x10, x11, [sp, #16 * 5]
            stp     x12, x13, [sp, #16 * 6]
            stp     x14, x15, [sp, #16 * 7]
            stp     x16, x17, [sp, #16 * 8]
            stp     x18, x19, [sp, #16 * 9]
            stp     x20, x21, [sp, #16 * 10]
            stp     x22, x23, [sp, #16 * 11]
            stp     x24, x25, [sp, #16 * 12]
            stp     x26, x27, [sp, #16 * 13]
            stp     x28, x29, [sp, #16 * 14]

            // Save x30 (Link Register) separately
            str     x30, [sp, #16 * 15]

            // Read and save Exception Link Register and Saved Program Status Register
            mrs     x0, elr_el1
            mrs     x1, spsr_el1
            stp     x0, x1, [sp, #16 * 16]
        .endm

        // macro_restore_context.S
        .macro RESTORE_CONTEXT
            // 1. Restore System Registers first
            ldp     x0, x1, [sp, #16 * 16]
            msr     elr_el1, x0
            msr     spsr_el1, x1

            // 2. Restore General Purpose Registers
            ldp     x0,  x1,  [sp, #16 * 0]
            ldp     x2,  x3,  [sp, #16 * 1]
            ldp     x4,  x5,  [sp, #16 * 2]
            ldp     x6,  x7,  [sp, #16 * 3]
            ldp     x8,  x9,  [sp, #16 * 4]
            ldp     x10, x11, [sp, #16 * 5]
            ldp     x12, x13, [sp, #16 * 6]
            ldp     x14, x15, [sp, #16 * 7]
            ldp     x16, x17, [sp, #16 * 8]
            ldp     x18, x19, [sp, #16 * 9]
            ldp     x20, x21, [sp, #16 * 10]
            ldp     x22, x23, [sp, #16 * 11]
            ldp     x24, x25, [sp, #16 * 12]
            ldp     x26, x27, [sp, #16 * 13]
            ldp     x28, x29, [sp, #16 * 14]

            // Restore x30 (Link Register)
            ldr     x30, [sp, #16 * 15]

            // 3. Shrink stack back to original position
            add     sp, sp, #272

            // Return to the address in elr_el1 with the state in spsr_el1
            eret
        .endm

        .macro SAVE_CONTEXT_MINIMAL
            // Make room for 22 registers (x0-x18, x29, x30, +1 for alignment)
            sub     sp, sp, #176

            // Save volatile registers in pairs
            stp     x0,  x1,  [sp, #16 * 0]
            stp     x2,  x3,  [sp, #16 * 1]
            stp     x4,  x5,  [sp, #16 * 2]
            stp     x6,  x7,  [sp, #16 * 3]
            stp     x8,  x9,  [sp, #16 * 4]
            stp     x10, x11, [sp, #16 * 5]
            stp     x12, x13, [sp, #16 * 6]
            stp     x14, x15, [sp, #16 * 7]
            stp     x16, x17, [sp, #16 * 8]

            // Save x18 and x29 (Frame Pointer)
            stp     x18, x29, [sp, #16 * 9]

            // Save x30 (Link Register)
            str     x30, [sp, #16 * 10]
        .endm
        .macro RESTORE_CONTEXT_MINIMAL
            // Restore x30 (Link Register)
            ldr     x30, [sp, #16 * 10]

            // Restore x18 and x29
            ldp     x18, x29, [sp, #16 * 9]

            // Restore x0 through x17
            ldp     x16, x17, [sp, #16 * 8]
            ldp     x14, x15, [sp, #16 * 7]
            ldp     x12, x13, [sp, #16 * 6]
            ldp     x10, x11, [sp, #16 * 5]
            ldp     x8,  x9,  [sp, #16 * 4]
            ldp     x6,  x7,  [sp, #16 * 3]
            ldp     x4,  x5,  [sp, #16 * 2]
            ldp     x2,  x3,  [sp, #16 * 1]
            ldp     x0,  x1,  [sp, #16 * 0]

            // Shrink stack
            add     sp, sp, #176

            eret
        .endm

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
            // mov x0, #5
            SAVE_CONTEXT
            bl {interrupt_handler}
            RESTORE_CONTEXT
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

unsafe extern "C" fn interrupt_handler(source: usize) {
    let esr = ESR_EL2.get();
    let elr = ELR_EL2.get();

    if let Some(interrupts) = INTERRUPTS.get() {
        let mut interrupts = interrupts.try_lock().unwrap();
        let irq_1_pending = interrupts.0.as_mut_ptr().irq_1_pending().read();
        info!("IRQ 1 Pending: {irq_1_pending:#b}");
        if irq_1_pending & (1 << 1) != 0 {
            info!("timer interrupt");
            let mut timer = TIMER.get().unwrap().lock();
            timer.clear_interrupt(u2::new(1));
        }
    }

    // panic!("interrupt / exception. source: {source}. esr: {esr:#X}. elr: {elr:#X}.");
}
