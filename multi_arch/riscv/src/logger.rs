use core::fmt::Write;

use log::{Log, max_level};

pub struct Logger;
impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        struct ConsoleWriter;
        impl Write for ConsoleWriter {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                for &byte in s.as_bytes() {
                    sbi::legacy::console_putchar(byte);
                }
                Ok(())
            }
        }
        let mut console = ConsoleWriter;
        writeln!(console, "{}", record.args()).unwrap();
    }

    fn flush(&self) {}
}

pub static LOGGER: Logger = Logger;

/// # Safety
/// Only call once.
pub unsafe fn init() {
    // Safety: nothing else is calling this function
    unsafe { log::set_logger_racy(&LOGGER).unwrap() };
    // Safety: nothing else is calling this function
    unsafe { log::set_max_level_racy(log::LevelFilter::Trace) };
}
