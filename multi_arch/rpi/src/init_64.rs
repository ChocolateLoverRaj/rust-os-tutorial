use core::{
    arch::{asm, naked_asm},
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
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

use crate::{RPI_3_PART_NO, RPI_4_PART_NO, init_common, logger};

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
static GOT_TIMER_1_INTERRUPT: AtomicBool = AtomicBool::new(false);
static GOT_TIMER_3_INTERRUPT: AtomicBool = AtomicBool::new(false);

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

    // Demonstrate using timers 1 and 3 to generate interrupts.
    // Timer 1 starts in 1s and generates an interrupt every 2s
    // Timer 2 starts in 2s and generates an interrupt every 2s
    // So every 1s we will get an interrupt and it will alternate between timer 1 and 3
    let pointer = NonNull::new((mmio_base + Timer::ADDRESS) as *mut Timer).unwrap();
    let mut timer = TimerRef(unsafe { VolatileRef::new(pointer) });
    let counter_lo = timer.counter_lo();
    let mut timer_1_compare_value = counter_lo.wrapping_add(1_000_000);
    let mut timer_3_compare_value = counter_lo.wrapping_add(2_000_000);
    timer.write_compare_value(u2::new(1), timer_1_compare_value);
    timer.clear_interrupt(u2::new(1));
    timer.write_compare_value(u2::new(3), timer_3_compare_value);
    timer.clear_interrupt(u2::new(3));

    let mut interrupts = InterruptsRef({
        let pointer = NonNull::new((mmio_base + Interrupts::ADDRESS) as *mut Interrupts).unwrap();
        unsafe { VolatileRef::new(pointer) }
    });
    interrupts.enable_irq(u6::new(1));
    interrupts.enable_irq(u6::new(3));

    info!("enabled timers 1 and 3");

    TIMER.call_once(|| Mutex::new(timer));
    INTERRUPTS.call_once(|| Mutex::new(interrupts));

    DAIF.set(0);

    loop {
        wfi();
        info!("after wfi");

        let timer_1_interrupt = GOT_TIMER_1_INTERRUPT.swap(false, Ordering::Relaxed);
        if timer_1_interrupt {
            info!("got timer 1 interrupt");
            timer_1_compare_value = timer_1_compare_value.wrapping_add(2_000_000);
            TIMER
                .get()
                .unwrap()
                .lock()
                .write_compare_value(u2::new(1), timer_1_compare_value);
        }
        let timer_3_interrupt = GOT_TIMER_3_INTERRUPT.swap(false, Ordering::Relaxed);
        if timer_3_interrupt {
            info!("got timer 3 interrupt");
            timer_3_compare_value = timer_3_compare_value.wrapping_add(2_000_000);
            TIMER
                .get()
                .unwrap()
                .lock()
                .write_compare_value(u2::new(3), timer_3_compare_value);
        }
    }
}

#[unsafe(naked)]
extern "C" fn vector_table() {
    naked_asm!(
        "
        .macro SAVE_CONTEXT
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
        .macro RESTORE_CONTEXT
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
        // Synchronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #0
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // IRQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #1
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // FIQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #2
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // Asynchronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #3
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // SP_ELx
        // Syncronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #4
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // IRQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #5
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // FIQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #6
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // Asynchronos Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #7
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // From lower EL
        // Synchronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #8
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // IRQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #9
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // FIQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #10
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // Asynchronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #11
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // From lower EL
        // Synchronous Exception
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #12
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // IRQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #13
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // FIQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #14
            bl {interrupt_handler}
            RESTORE_CONTEXT
        // FIQ
        .balign 0x80
            SAVE_CONTEXT
            mov x0, #15
            bl {interrupt_handler}
            RESTORE_CONTEXT
        ",
        interrupt_handler = sym interrupt_handler,
    )
}

unsafe extern "C" fn interrupt_handler(source: usize) {
    let esr = ESR_EL2.get();
    let elr = ELR_EL2.get();
    info!("interrupt / exception. source: {source}. esr: {esr:#X}. elr: {elr:#X}.");

    if let Some(interrupts) = INTERRUPTS.get() {
        let mut interrupts = interrupts.try_lock().unwrap();
        let irq_1_pending = interrupts.0.as_mut_ptr().irq_1_pending().read();
        info!("IRQ 1 Pending: {irq_1_pending:#b}");
        if irq_1_pending & (1 << 1) != 0 {
            info!("timer 1 interrupt");
            let mut timer = TIMER.get().unwrap().lock();
            timer.clear_interrupt(u2::new(1));
            GOT_TIMER_1_INTERRUPT.store(true, Ordering::Relaxed);
        }
        if irq_1_pending & (1 << 3) != 0 {
            info!("timer 3 interrupt");
            let mut timer = TIMER.get().unwrap().lock();
            timer.clear_interrupt(u2::new(3));
            GOT_TIMER_3_INTERRUPT.store(true, Ordering::Relaxed);
        }
    }
}
