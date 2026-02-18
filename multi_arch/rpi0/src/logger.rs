use core::fmt::Write;

use arm_pl011_uart::Uart;
use log::{LevelFilter, Log, max_level, set_logger, set_max_level};
use spin::{Once, mutex::Mutex};

struct Logger {
    uart: Mutex<Option<Uart<'static>>>,
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        let mut uart = self.uart.lock();
        if let Some(uart) = uart.as_mut() {
            writeln!(uart, "{}", record.args()).unwrap();
        } else {
            #[cfg(feature = "semihosting")]
            semihosting::println!("{}", record.args());
        };
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger {
    uart: Mutex::new(None),
};

pub fn init() {
    set_logger(&LOGGER).unwrap();
    set_max_level(LevelFilter::Trace);
}

pub fn init_uart(uart: Uart<'static>) {
    *LOGGER.uart.lock() = Some(uart);
}
