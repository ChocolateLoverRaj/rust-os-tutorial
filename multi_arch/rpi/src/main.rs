#![no_std]
#![no_main]
#![feature(stdarch_arm_hints)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_neon_intrinsics))]

mod halt_loop;
mod logger;

use core::{arch::naked_asm, panic::PanicInfo, ptr::NonNull};

use aarch32_cpu::register::Midr;
use arbitrary_int::u12;
use arm_pl011_uart::{Uart, UniqueMmioPointer};
use ez_mailbox::{
    Mailbox, MailboxVolatileFieldAccess, board_revision, call, get_board_revision,
    volatile::VolatileRef,
};
use fdt::{Fdt, properties::Compatible};
use log::{error, info};
use phf::phf_map;

use crate::{halt_loop::halt_loop, logger::init_uart};

unsafe extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    error!("{panic_info}");
    halt_loop()
}

static BUNDLED_DEVICE_TREES: phf::Map<usize, &[u8]> = phf_map! {
    0xC42 => include_bytes!("../bcm2708-rpi-zero.dtb")
};

pub fn init_common(part_no: u12) {
    // Figure out what machine we are running on
    // https://wiki.osdev.org/Detecting_Raspberry_Pi_Board
    match part_no {
        i if i == u12::new(0xB76) => {
            info!("Running on a Raspberry Pi Zero or 1");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x20000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi Zero or 1");
        }
        i if i == u12::new(0xC07) => {
            info!("Running on a Raspberry Pi 2");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x3F000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi 2");
        }
        i if i == u12::new(0xD03) => {
            info!("Running on a Raspberry Pi 3");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x3F000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi 3");
        }
        i if i == u12::new(0xD08) => {
            info!("Running on a Raspberry Pi 4");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0xFE000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi 4");
        }
        part_no => {
            info!("Unknown part number: {part_no:#X}. Not doing anything.");
        }
    }
}

#[cfg(target_arch = "arm")]
extern "C" fn kernel_main_32(_r0: usize, _machine_id: usize, _atags_ptr: usize) -> ! {
    use aarch32_cpu::{asm::irq_enable, register::Cpsr, svc};

    logger::init();

    info!(
        "Hello from Rust kernel booted on 32 bit ARM. This is likely booted on a Raspberry Pi Zero, 1, or 2."
    );
    let midr = Midr::read();
    init_common(midr.part_no());
    // if let Some(device_tree) = BUNDLED_DEVICE_TREES.get(&machine_id) {
    //     let fdt = Fdt::new_unaligned(device_tree).unwrap();
    //     let chosen = fdt.root().chosen();
    //     if let Some(stdout) = chosen.stdout() {
    //         info!("found stdout: {:#?}", stdout);
    //         let compatible = stdout.node.property::<Compatible>().unwrap();
    //         info!("serial compatible: {compatible:?}");
    //         for range in stdout.node.reg().unwrap().iter::<u32, u32>() {
    //             info!("reg: {range:X?}");
    //         }
    //         let mut node = stdout.node;
    //         loop {
    //             if let Some(parent) = node.parent() {
    //                 info!("parent: {parent:?}");
    //                 if let Some(ranges) = node.ranges() {
    //                     info!("has ranges");
    //                     for range in ranges.iter::<usize, usize, usize>() {
    //                         info!("range: {range:?}");
    //                     }
    //                 } else {
    //                     info!("does not have ranges");
    //                 }
    //                 node = parent;
    //             } else {
    //                 info!("no parent");
    //                 break;
    //             }
    //         }

    //         if stdout
    //             .node
    //             .property::<Compatible>()
    //             .unwrap()
    //             .compatible_with("arm,pl011-axi")
    //         {
    //             info!("compatible!");
    //             // FIXME: Proper address translation
    //             let address = NonNull::new(
    //                 (stdout
    //                     .node
    //                     .reg()
    //                     .unwrap()
    //                     .iter::<u32, u32>()
    //                     .next()
    //                     .unwrap()
    //                     .unwrap()
    //                     .address
    //                     - 0x7e000000
    //                     + 0x20000000) as usize as *mut _,
    //             )
    //             .unwrap();
    //             info!("address: {address:?}");

    //             let mut uart = Uart::new(unsafe { UniqueMmioPointer::new(address) });
    //             uart.write_str("Hello from UART \"driver\"\n").unwrap();
    //         }
    //     } else {
    //         info!("no stdout!");
    //     }
    // } else {
    //     error!("No device tree for machine id. Not doing anything.");
    // }

    // let atags_ptr = NonNull::new(atags_ptr as *mut _).unwrap();
    // // Safety: the kernel was given a valid ptr to ATAGS through r2
    // let mut atags = unsafe { Atags::new(atags_ptr) };
    // for atag in atags.iter() {}

    let vectors_addr = vectors as *const () as usize;
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {0}, c12, c0, 0", // Write to VBAR
            in(reg) vectors_addr
        );
    }

    info!("wrote to VBAR {vectors_addr:#X}");

    // Set the stack pointer of the UND handler
    unsafe {
        core::arch::asm!(
            // 1. Change the CPSR mode bits to 0x1B (Undefined Mode)
            // We use 0xDB to also keep interrupts disabled (I and F bits)
            "msr cpsr_c, #0xdb",
            // 2. Now 'sp' refers to the banked SP_und register
            "ldr sp, =__und_stack_top",
            // 3. Switch back to Supervisor Mode (0xD3)
            "msr cpsr_c, #0xd3",
        );
    }
    info!("set stack pointer of UND handler");

    unsafe {
        core::arch::asm!(
            "mrs r0, cpsr",      // Save current (SVC) mode
            "msr cpsr_c, #0xd2", // Switch to IRQ Mode (0x12 | 0xC0)
            "ldr sp, =__und_stack_top",   // Set a 4KB-aligned stack pointer
            "msr cpsr_c, r0",    // Switch back to SVC Mode
            out("r0") _,
            options(nomem, nostack)
        );
    }

    // svc!(3);
    // unsafe {
    //     core::arch::asm!(".word 0xe7f000f0");
    // }
    // info!("did undefined instruction");
    unsafe { irq_enable() };

    const IRQ_ENABLE_1: *mut u32 = 0x2000_B210 as *mut u32;

    unsafe {
        // Enable IRQ 1 (System Timer Compare 1)
        core::ptr::write_volatile(IRQ_ENABLE_1, 1 << 1);
    }

    const TIMER_CS: *mut u32 = 0x2000_3000 as *mut u32;

    unsafe {
        // Write a 1 to bit 1 to CLEAR any pending interrupt for Compare 1
        core::ptr::write_volatile(TIMER_CS, 1 << 1);
    }

    const TIMER_CLO: *mut u32 = 0x2000_3004 as *mut u32; // Lower 32 bits of counter
    const TIMER_C1: *mut u32 = 0x2000_3010 as *mut u32; // Compare register 1

    unsafe {
        // 1. Read current time
        let current_val = core::ptr::read_volatile(TIMER_CLO);

        // 2. Set match for 1 second from now (System Timer runs at 1MHz)
        let match_val = current_val.wrapping_add(1_000_000);

        // 3. Write to Compare 1
        core::ptr::write_volatile(TIMER_C1, match_val);
    }

    info!("enabled the timer");

    halt_loop()
}

#[cfg(target_arch = "aarch64")]
fn kernel_main_64(dtb_ptr: u32, _x1: usize, _x2: usize, _x3: usize) -> ! {
    use aarch64_cpu::registers::{MIDR_EL1, Readable};

    logger::init();

    info!(
        "Hello from Rust kernel booted on 64 bit ARM. This is likely booted on a Raspberry Pi 3 or 4. Device Tree pointer: {dtb_ptr:#X}."
    );

    // TODO: Use device tree
    let midr = MIDR_EL1.get();
    init_common(u12::from_u64(MIDR_EL1::PartNum.read(midr)));

    halt_loop()
}

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

#[cfg(target_arch = "aarch64")]
#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Entry point for the kernel. Registers:
        // x0 -> 32 bit pointer to DTB in memory (primary core only) / 0 (secondary cores)
        // x1 -> 0
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

#[cfg(target_arch = "arm")]
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

#[unsafe(naked)]
extern "C" fn reset_handler() {
    naked_asm!("todo: b todo")
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
    halt_loop()
}
extern "C" fn fiq_handler() {
    info!("fiq handler");
    halt_loop()
}
