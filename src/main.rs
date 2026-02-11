#![no_std]
#![no_main]
mod sbi;

use core::{
    arch::{asm, naked_asm},
    fmt::Write,
    panic::PanicInfo,
    ptr::write_bytes,
};
use sbi::*;

use log::max_level;

unsafe extern "C" {
    static __bss: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

#[panic_handler]
fn panic_handler(_panic_info: &PanicInfo) -> ! {
    loop {}
}

unsafe fn put_char(char: u32) {
    unsafe {
        sbi_call(
            char as usize,
            0,
            0,
            0,
            0,
            0,
            0,
            1, /* Console Putchar */
        )
    };
}

struct ConsoleWriter;
impl Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for char in s.chars() {
            unsafe { put_char(char.into()) };
        }
        Ok(())
    }
}

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        let mut writer = ConsoleWriter;
        writeln!(writer, "{}", record.args()).unwrap();
    }

    fn flush(&self) {}
}
static LOGGER: Logger = Logger;

extern "C" fn kernel_main(hart_id: usize, ftd_ptr: usize) {
    unsafe { write_bytes(__bss as *mut u8, 0, __bss_end - __bss) };

    // Safety: nothing else is calling this function
    unsafe { log::set_logger_racy(&LOGGER).unwrap() };
    // Safety: nothing else is calling this function
    unsafe { log::set_max_level_racy(log::LevelFilter::Trace) };

    let spi_spec_version =
        log::info!("Hello from Rust!. HART ID: {hart_id}. Device Tree pointer: {ftd_ptr:#X}.");

    loop {}
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
