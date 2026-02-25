use core::fmt::Write;

use log::{LevelFilter, Log, max_level, set_logger, set_max_level};

use crate::UART;

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        critical_section::with(|_cs| {
            if let Some(uart) = UART.get() {
                let mut uart = uart.lock();
                writeln!(uart, "{}", record.args()).unwrap();
            } else {
                #[cfg(feature = "semihosting")]
                semihosting::println!("{}", record.args());
            };
        });
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

pub fn init() {
    set_logger(&LOGGER).unwrap();
    set_max_level(LevelFilter::Trace);
}
