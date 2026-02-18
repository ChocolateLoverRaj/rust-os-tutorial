#![no_std]
#![no_main]
#![feature(stdarch_arm_hints)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_neon_intrinsics))]

mod halt_loop;
#[cfg(target_arch = "arm")]
mod init_32;
#[cfg(target_arch = "aarch64")]
mod init_64;
mod logger;

use core::{panic::PanicInfo, ptr::NonNull};

use arbitrary_int::u12;
use arm_pl011_uart::{Uart, UniqueMmioPointer};
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

const RPI_0_1_PART_NO: u16 = 0xB76;
const RPI_2_PART_NO: u16 = 0xC07;
const RPI_3_PART_NO: u16 = 0xD03;
const RPI_4_PART_NO: u16 = 0xD08;

static MMIO_BASE: phf::Map<u16, usize> = phf_map! {
    0xB76 => 0x20000000,
    0xC07 => 0x3F000000,
    0xD03 => 0x3F000000,
    0xD08 => 0xFE000000,
};

pub fn init_common(part_no: u12) {
    // Figure out what machine we are running on
    // https://wiki.osdev.org/Detecting_Raspberry_Pi_Board
    match part_no {
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
        i if i == u12::new(RPI_3_PART_NO) => {
            info!("Running on a Raspberry Pi 3");
            // It's safe to use this UART on any computer after this check.
            const MMIO_BASE: usize = 0x3F000000;
            const GPIO_BASE: usize = MMIO_BASE + 0x200000;
            const UART0_BASE: usize = GPIO_BASE + 0x1000;
            let ptr = NonNull::new(UART0_BASE as *mut _).unwrap();
            init_uart(Uart::new(unsafe { UniqueMmioPointer::new(ptr) }));
            info!("Running on a Raspberry Pi 3");
        }
        i if i == u12::new(RPI_4_PART_NO) => {
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
