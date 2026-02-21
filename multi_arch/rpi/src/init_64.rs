use core::{
    arch::{asm, naked_asm},
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

use aarch64_cpu::{
    asm::{eret, wfi},
    registers::{
        CNTHCTL_EL2, CNTVOFF_EL2, CurrentEL, DAIF, ELR_EL1, ELR_EL2, ESR_EL1, ESR_EL2, HCR_EL2,
        ICC_SRE_EL2, MIDR_EL1, MPIDR_EL1, Readable, SP_EL1, SPSR_EL2, VBAR_EL1, VBAR_EL2,
        Writeable,
    },
};
use arbitrary_int::{u2, u6, u12};
use arm_gic::{
    IntId,
    gicv2::{GicV2, SgiTarget},
};
use arm_gic_driver::VirtAddr;
use ez_mailbox::{
    interrupts::{Interrupts, InterruptsRef, InterruptsVolatileFieldAccess},
    timer::{Timer, TimerRef},
    volatile::VolatileRef,
};
use log::info;
use spin::{Mutex, Once};

use crate::{__stack_top, RPI_3_PART_NO, RPI_4_PART_NO, init_common, logger};

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        halt: b halt
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
        kernel_main = sym kernel_main
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

fn kernel_main(dtb_ptr: u32, _x1: usize, _x2: usize, _x3: usize) -> ! {
    logger::init();

    info!(
        "Hello from Rust kernel booted on 64 bit ARM. This is likely booted on a Raspberry Pi 3 or 4. Device Tree pointer: {dtb_ptr:#X}."
    );

    // TODO: Use device tree
    let midr = MIDR_EL1.get();
    init_common(u12::from_u64(MIDR_EL1::PartNum.read(midr)));

    let current_el = CurrentEL::EL.read(CurrentEL.get());
    info!("current el: {current_el}");
    if current_el == 2 {
        // Enable timer counter registers for EL1.
        CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);

        // No offset for reading the counters.
        CNTVOFF_EL2.set(0);

        // Set EL1 execution state to AArch64.
        HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);

        // Set up a simulated exception return.
        //
        // First, fake a saved program status where all interrupts were masked and SP_EL1 was used as a
        // stack pointer.
        SPSR_EL2.write(
            SPSR_EL2::D::Masked
                + SPSR_EL2::A::Masked
                + SPSR_EL2::I::Masked
                + SPSR_EL2::F::Masked
                + SPSR_EL2::M::EL1h,
        );

        // Second, let the link register point to kernel_init().
        ELR_EL2.set(kernel_main_el1 as *const () as u64);

        // Set up SP_EL1 (stack pointer), which will be used by EL1 once we "return" to it. Since there
        // are no plans to ever return to EL2, just re-use the same stack.
        SP_EL1.set(core::ptr::from_ref(unsafe { &__stack_top }).addr() as u64);

        eret();
    } else {
        panic!("Unexpected EL");
    }

    // let mut hcr_el2 = HCR_EL2.get();
    // info!("HCR: {hcr_el2:#X}");
    // hcr_el2 |= 1 << 4;
    // HCR_EL2.set(hcr_el2);

    // let interrupt_handler_stack_top =
    //     core::ptr::from_ref(unsafe { &__interrupt_handler_stack_top }).addr() as u64;
    // info!("setting interrupt handler stack top: {interrupt_handler_stack_top:#X}");
    // let sp_sel = SPSel.get();
    // info!("SPSel: {sp_sel:#b}");
    // let sp_el2 = SP_EL2.get();
    // info!("SP_EL2: {sp_el2:#X}");
    // SP_EL2.set(interrupt_handler_stack_top);
    // DAIF.set(0);

    // info!("testing HVC exception");
    // unsafe { asm!("hvc #0") };
    // let a = 0xFF800040 as *mut u32;
    // unsafe { a.write_volatile(0x100) };
    // info!("a: {:#X}", unsafe { a.read_volatile() });
}

extern "C" fn kernel_main_el1() -> ! {
    let current_el = CurrentEL::EL.read(CurrentEL.get());
    info!("Expected current EL: EL1. current el: {current_el}");

    let vector_table_addr = vector_table as *const () as usize;
    VBAR_EL1.set(vector_table_addr as u64);
    info!("set vector table addr: {vector_table_addr:#X}.");

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

    {
        const GIC_BASE: usize = 0xFF840000;
        const GICD_DIST_BASE: usize = GIC_BASE + 0x00001000;
        const GICC_CPU_BASE: usize = GIC_BASE + 0x00002000;

        const GICD_ENABLE_IRQ_BASE: usize = GICD_DIST_BASE + 0x00000100;

        const GICC_IAR: usize = GICC_CPU_BASE + 0x0000000C;
        const GICC_EOIR: usize = GICC_CPU_BASE + 0x00000010;

        const GIC_IRQ_TARGET_BASE: usize = GICD_DIST_BASE + 0x00000800;

        const SYSTEM_TIMER_IRQ_1: usize = 0x61;

        fn enable_interrupt(irq: usize) {
            let n = irq / 32;
            let offset = irq % 32;
            let ptr = (GICD_ENABLE_IRQ_BASE + (4 * n)) as *mut u32;
            let reg = unsafe { ptr.read_volatile() };
            unsafe { ptr.write_volatile(reg | (1 << offset)) };
        }

        fn assign_target(irq: usize, cpu: usize) {
            let n = irq / 4;
            let byte_offset = irq % 4;
            let ptr = (GIC_IRQ_TARGET_BASE + (4 * n)) as *mut u32;
            let reg = unsafe { ptr.read_volatile() };
            let shift = byte_offset * 8 + cpu;
            // Currently we only enter the target CPU 0
            unsafe { ptr.write(reg | (1 << shift)) };
        }

        const GICC_CTLR: usize = GICC_CPU_BASE + 0x000;
        const GICC_PMR: usize = GICC_CPU_BASE + 0x004;

        fn enable_interrupt_controller() {
            enable_interrupt(SYSTEM_TIMER_IRQ_1);
            assign_target(SYSTEM_TIMER_IRQ_1, 0);
            unsafe {
                (GICC_PMR as *mut u32).write_volatile(0xFF); // allow all priorities
                (GICC_CTLR as *mut u32).write_volatile(1); // enable signaling
            }
        }

        if MIDR_EL1::PartNum.read(MIDR_EL1.get()) as u16 == RPI_4_PART_NO {
            info!("enabling rpi4 interrupt controller");
            let mut gic = unsafe {
                arm_gic_driver::v2::Gic::new(
                    VirtAddr::new(GICD_DIST_BASE),
                    VirtAddr::new(GICC_CPU_BASE),
                    None,
                )
            };
            gic.init();
            gic.cpu_interface().init_current_cpu();
            // Enable an Timer interrupt
            let irq_id = arm_gic_driver::IntId::spi(SYSTEM_TIMER_IRQ_1 as u32 - 32);
            gic.set_irq_enable(irq_id, true);

            // Set interrupt priority
            gic.set_priority(irq_id, 0x80);

            // enable_interrupt_controller();

            // let mut gic = unsafe { GicV2::new(GICD_DIST_BASE as *mut _, GICC_CPU_BASE as *mut _) };
            // // gic.setup();
            // // gic.enable_all_interrupts(true);
            // // gic.set_interrupt_priority(IntId::spi(96 - 32), 0);
            // for spi in 96..96 + 4 {
            //     gic.enable_interrupt(IntId::spi(spi - 32), true).unwrap();
            //     gic.set_interrupt_priority(IntId::spi(spi - 32), 0);
            // }
            // // gic.enable_interrupt(IntId::sgi(0), true).unwrap();
            // // gic.set_interrupt_priority(IntId::sgi(0), 0);
            // // gic.set_priority_mask(u8::MAX);
            // // gic.send_sgi(IntId::sgi(0), SgiTarget::All);
            // info!("types: {:#?}", gic.typer().cpu_count());
            // let cpu_id = MPIDR_EL1::Aff0.read(MPIDR_EL1.get());
            // info!("CPU id: {cpu_id:#X}");
        } else {
            info!("not rpi4");
        }
    }
    unsafe {
        asm!(
            "
            dsb sy
            "
        )
    };

    DAIF.set(0);
    // unsafe { asm!("msr daifclr, #2") }

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
    let esr = ESR_EL1.get();
    let elr = ELR_EL1.get();
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
