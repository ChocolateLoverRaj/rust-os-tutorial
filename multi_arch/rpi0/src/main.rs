#![no_std]
#![no_main]
#![feature(stdarch_arm_hints, stdarch_arm_neon_intrinsics)]

mod logger;

use core::arch::naked_asm;
use core::arch::{arm::__wfe, asm};
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::NonNull;

use arm_pl011_uart::{Uart, UniqueMmioPointer};
// #[cfg(feature = "semihosting")]
// use defmt_semihosting as _;
use fdt::Fdt;
use fdt::properties::Compatible;
use log::{error, info};
use phf::phf_map;

unsafe extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    error!("{panic_info}");
    loop {
        unsafe { __wfe() };
    }
}

static BUNDLED_DEVICE_TREES: phf::Map<usize, &[u8]> = phf_map! {
    0xC42 => include_bytes!("../bcm2708-rpi-zero.dtb")
};

/// Reads the Main ID Register (MIDR) from Coprocessor 15.
/// Returns a 32-bit value containing the CPU implementer, variant, and part number.
pub fn read_midr() -> usize {
    let reg: usize;
    unsafe {
        asm!(
            "mrc p15, 0, {0}, c0, c0, 0",
            out(reg) reg,
            options(nomem, nostack, preserves_flags)
        );
    }
    reg
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(_r0: usize, machine_id: usize, atags_ptr: usize) -> ! {
    logger::init();
    info!("Hello from Rust kernel. Machine id: {machine_id:#X}. ATAGs ptr: {atags_ptr:#X}.");
    let midr = read_midr();
    let part_number = ((midr >> 4) & 0xFFF) as u16;
    info!("Main ID: {midr:#X}. Part number: {part_number:#X}");
    let magic = unsafe { (atags_ptr as *const [u32; 2]).read() };
    info!("ATAGS / Device Tree magic: {magic:#X?}");

    // error!("hello QEMU");
    // error!("hello QEMU");
    // error!("hello QEMU");
    // error!("hello QEMU");
    if let Some(device_tree) = BUNDLED_DEVICE_TREES.get(&machine_id) {
        let fdt = Fdt::new_unaligned(device_tree).unwrap();
        let chosen = fdt.root().chosen();
        if let Some(stdout) = chosen.stdout() {
            info!("found stdout: {:#?}", stdout);
            let compatible = stdout.node.property::<Compatible>().unwrap();
            info!("serial compatible: {compatible:?}");
            for range in stdout.node.reg().unwrap().iter::<u32, u32>() {
                info!("reg: {range:X?}");
            }
            let mut node = stdout.node;
            loop {
                if let Some(parent) = node.parent() {
                    info!("parent: {parent:?}");
                    if let Some(ranges) = node.ranges() {
                        info!("has ranges");
                        for range in ranges.iter::<usize, usize, usize>() {
                            info!("range: {range:?}");
                        }
                    } else {
                        info!("does not have ranges");
                    }
                    node = parent;
                } else {
                    info!("no parent");
                    break;
                }
            }

            if stdout
                .node
                .property::<Compatible>()
                .unwrap()
                .compatible_with("arm,pl011-axi")
            {
                info!("compatible!");
                // FIXME: Proper address translation
                let address = NonNull::new(
                    (stdout
                        .node
                        .reg()
                        .unwrap()
                        .iter::<u32, u32>()
                        .next()
                        .unwrap()
                        .unwrap()
                        .address
                        - 0x7e000000
                        + 0x20000000) as usize as *mut _,
                )
                .unwrap();
                info!("address: {address:?}");

                let mut uart = Uart::new(unsafe { UniqueMmioPointer::new(address) });
                uart.write_str("Hello from UART \"driver\"\n").unwrap();
            }
        } else {
            info!("no stdout!");
        }
    } else {
        error!("No device tree for machine id. Not doing anything.");
    }

    // let atags_ptr = NonNull::new(atags_ptr as *mut _).unwrap();
    // // Safety: the kernel was given a valid ptr to ATAGS through r2
    // let mut atags = unsafe { Atags::new(atags_ptr) };
    // for atag in atags.iter() {}
    loop {
        unsafe { __wfe() };
    }
}

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
        kernel_main = sym kernel_main
    )
}
