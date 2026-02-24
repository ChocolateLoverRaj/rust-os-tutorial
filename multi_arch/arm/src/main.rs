#![no_std]
#![no_main]
#![feature(stdarch_arm_hints)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_neon_intrinsics))]

mod logger;

use core::arch::arm::{__wfe, __wfi};
use core::arch::naked_asm;
use core::ptr::addr_of;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{panic::PanicInfo, ptr::NonNull};

use aarch32_cpu::interrupt::disable;
use aarch32_cpu::register::cpsr::ProcessorMode;
use aarch32_cpu::register::{Cpsr, Midr};
use arbitrary_int::{u2, u6, u12};
use arm_pl011_uart::{Uart, UniqueMmioPointer};
use ez_mailbox::interrupts::{Interrupts, InterruptsRef};
use ez_mailbox::timer::{Timer, TimerRef};
use ez_mailbox::volatile::VolatileRef;
use log::{error, info};
use spin::Once;

use crate::logger::init_uart;

unsafe extern "C" {
    static __interrupt_handler_stack_top: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    disable();
    unsafe { logger::force_unlock() };
    error!("{panic_info}");
    loop {
        unsafe { __wfe() };
    }
}

#[cfg(all(target_arch = "arm", not(target_feature = "v7")))]
#[unsafe(link_section = ".text._header")]
// Prevent this function from being removed
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() {
    naked_asm!(
        "
        // Get the addres where the start of our kernel was loaded
        sub r3, pc, #8
        b {start_common}
        ",
        start_common = sym start_common
    )
}

#[cfg(all(target_arch = "arm", target_feature = "v7"))]
#[unsafe(link_section = ".text._header")]
// Prevent this function from being removed
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Get the addres where the start of our kernel was loaded
        sub r3, pc, #8

        // Shut off extra cores
        // Read the core id
        mrc p15, 0, r5, c0, c0, 5
        and r5, r5, #3
        // Halt loop if the core id is not 0
        cmp r5, #0
	    bne halt

		b {start_common}

        halt:
            wfe
            b halt
        ",
        start_common = sym start_common
    )
}

#[unsafe(naked)]
extern "C" fn start_common() {
    naked_asm!(
        "
        // Apply relocations
        // Get the actual ptr to start of relocations
        ldr r4, =__rel_start
        add r4, r4, r3
        // Get the actual ptr to end of relocations
        ldr r5, =__rel_end
        add r5, r5, r3
        // Get the difference in (actual address we're loaded in - the initial address relocations are based on)
        // Since our default ELF start address is 0x0, the diference is (r3 - 0 = r3), so we can just use r3

        // Loop through all relocations. Each relocation is a (u32, u32).
        // The first u32 is an offset from the start of the kernel binary file to the pointer in memory we need to update
        // The second u32 contains the relocation type in the lower byte
        .reloc_loop:
            // Exit the loop once we're done
            cmp r4, r5
            beq .reloc_done

            // Make sure the relocation is R_ARM_RELATIVE
            // So far this is the only type of relocation our binary has
            // But we can handle other types if they show up
            // Load the second u32
            ldr r7, [r4, #4]
            // We want to know the type which is stored in the lower 8 bits
            and r7, r7, #0xff
            cmp r7, #0x17
            bne .unknown_reloc

            // Load the first u32
            ldr r7, [r4]
            // Get the real address of that location in our memory by adjusting by the offset
            add r7, r7, r3
            // Load the pointer that needs to be adjusted
            ldr r8, [r7]
            // Adjust the pointer
            add r8, r8, r3
            // Store the updated value
            str r8, [r7]

            // Go to the next relocation
            add r4, r4, #8
            b .reloc_loop

        .unknown_reloc:
             b .unknown_reloc

        .reloc_done:

        // Set the stack pointer to the stack space we reserved in the linker script
        ldr sp, =__stack_top
        add sp, sp, r3

        // Zero the BSS. Zero it by 4 * usize at a time instead of one byte or one usize at a time
        ldr r4, =__bss_start
        add r4, r4, r3
        ldr r9, =__bss_end
        add r9, r9, r3
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
        kernel_main = sym kernel_main,
    )
}

const RPI_0_1_PART_NO: u16 = 0xB76;
const RPI_2_PART_NO: u16 = 0xC07;

static TIMER: Once<TimerRef> = Once::new();
static INTERRUPTS: Once<InterruptsRef> = Once::new();

static TIMER_1_COMPLETE: AtomicBool = AtomicBool::new(false);
static TIMER_3_COMPLETE: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
extern "C" fn kernel_main(
    _r0: usize,
    _machine_id: usize,
    atags_or_fdt_ptr: usize,
    kernel_start: usize,
) -> ! {
    logger::init();

    info!(
        "Hello from Rust kernel booted on 32 bit ARM. ATAGS / FDT ptr: {atags_or_fdt_ptr:#X}. Kernel loaded at address: {kernel_start:#X}"
    );
    let midr = Midr::read();
    let vectors_addr = vectors as *const () as usize;
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {0}, c12, c0, 0", // Write to VBAR
            in(reg) vectors_addr
        );
    }

    // Figure out what machine we are running on
    // https://wiki.osdev.org/Detecting_Raspberry_Pi_Board
    match midr.part_no() {
        i if i == u12::new(RPI_0_1_PART_NO) => {
            info!("Running on a Raspberry Pi Zero or 1");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x20000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi Zero or 1");
        }
        i if i == u12::new(RPI_2_PART_NO) => {
            info!("Running on a Raspberry Pi 2");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x3F000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi 2");
        }
        part_no => {
            info!("Unknown part number: {part_no:#X}. Not doing anything.");
        }
    }

    info!("wrote to VBAR {vectors_addr:#X}");

    let interrupt_handler_stack_top = kernel_start + addr_of!(__interrupt_handler_stack_top).addr();
    // Set the stack pointer of the UND handler
    unsafe {
        core::arch::asm!(
            "
            // 1. Change the CPSR mode bits to 0x1B (Undefined Mode)
            // We use 0xDB to also keep interrupts disabled (I and F bits)
            msr cpsr_c, #0xdb
            // 2. Now 'sp' refers to the banked SP_und register
            mov sp, {}
            // 3. Switch back to Supervisor Mode (0xD3)
            msr cpsr_c, #0xd3
            ",
            in(reg) interrupt_handler_stack_top
        );
    }

    // Set the stack pointer of the IRQ handler
    unsafe {
        core::arch::asm!(
            "
            // Save current (SVC) mode
            mrs r0, cpsr
            // Switch to IRQ Mode (0x12 | 0xC0)
            msr cpsr_c, #0xd2
            // Set a 4KB-aligned stack pointer
            mov sp, {}
            // Switch back to SVC Mode
            msr cpsr_c, r0
            ",
            in(reg) interrupt_handler_stack_top,
            // Mark r0 as modified by our assembly
            out("r0") _,
            options(nomem, nostack)
        );
    }

    let mmio_base = get_mmio_base();
    let timer = TimerRef({
        let ptr = NonNull::new((mmio_base + Timer::ADDRESS) as *mut _).unwrap();
        unsafe { VolatileRef::new(ptr) }
    });
    let counter_lo = timer.counter_lo();
    let mut timer_1_compare_value = counter_lo.wrapping_add(1_000_000);
    timer.write_compare_value(u2::new(1), timer_1_compare_value);
    timer.clear_interrupt(u2::new(1));
    let mut timer_3_compare_value = counter_lo.wrapping_add(2_000_000);
    timer.write_compare_value(u2::new(3), timer_3_compare_value);
    timer.clear_interrupt(u2::new(3));

    let interrupts = InterruptsRef({
        let ptr = NonNull::new((mmio_base + Interrupts::ADDRESS) as *mut _).unwrap();
        unsafe { VolatileRef::new(ptr) }
    });
    interrupts.enable_irq(u6::new(1));
    interrupts.enable_irq(u6::new(3));

    info!("enabled timers 1 and 3");
    let timer = TIMER.call_once(|| timer);
    INTERRUPTS.call_once(|| interrupts);

    // Safety: we're not in an interrupt handler
    unsafe { aarch32_cpu::interrupt::enable() };

    loop {
        unsafe { __wfi() };
        if TIMER_1_COMPLETE.swap(false, Ordering::Relaxed) {
            info!("timer 1 complete");
            timer_1_compare_value = timer_1_compare_value.wrapping_add(2_000_000);
            timer.write_compare_value(u2::new(1), timer_1_compare_value);
        }
        if TIMER_3_COMPLETE.swap(false, Ordering::Relaxed) {
            info!("timer 3 complete");
            timer_3_compare_value = timer_3_compare_value.wrapping_add(2_000_000);
            timer.write_compare_value(u2::new(3), timer_3_compare_value);
        }
    }
}

fn get_mmio_base() -> usize {
    match Midr::read().part_no() {
        i if i == u12::new(RPI_0_1_PART_NO) => 0x20000000,
        i if i == u12::new(RPI_2_PART_NO) => 0x3F000000,
        _ => panic!("unknown part number"),
    }
}

#[unsafe(naked)]
extern "C" fn vectors() {
    naked_asm!(
        "
        .balign 32
        b exception_handler
        b exception_handler
        b exception_handler
        b exception_handler
        b exception_handler
        b . // reserved
        b exception_handler
        b . // FIQ

        exception_handler:
            sub lr, lr, #4
            stmfd   sp!, {{r0-r12, lr}}
            bl {exception_handler}
            ldmfd   sp!, {{r0-r12, pc}}^
        ",
        exception_handler = sym exception_handler,
    )
}

unsafe extern "C" fn exception_handler() {
    let cpsr = Cpsr::read();
    if cpsr.mode() == Ok(ProcessorMode::Irq) {
        let interrupts = INTERRUPTS.get().unwrap();
        let pending_irqs = interrupts.pending_interrupts_irq_0_32();
        // info!("pending irqs: {pending_irqs:#X}");
        let timer = TIMER.get().unwrap();
        if pending_irqs & (1 << 1) != 0 {
            // info!("timer 1 done");
            timer.clear_interrupt(u2::new(1));
            TIMER_1_COMPLETE.store(true, Ordering::Relaxed);
        }
        if pending_irqs & (1 << 3) != 0 {
            // info!("timer 3 done");
            timer.clear_interrupt(u2::new(3));
            TIMER_3_COMPLETE.store(true, Ordering::Relaxed);
        }
    } else {
        panic!("exception in unexpected processor mode. cpsr: {cpsr:?}")
    }
}
