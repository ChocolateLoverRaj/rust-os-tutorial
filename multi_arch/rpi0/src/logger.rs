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
        let mut writer: Option<&mut dyn Write> = if let Some(uart) = self.uart.) {
            Some()
        } else {
            #[cfg(feature = "semihosting")]
            semihosting::println!("{}", record.args());
        };
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger { uart: Once::new() };

pub fn init() {
    set_logger(&LOGGER).unwrap();
    set_max_level(LevelFilter::Trace);
}

pub fn init_uart(uart: Uart<'static>) {
    *LOGGER.uart.lock() = Some(uart);
}
