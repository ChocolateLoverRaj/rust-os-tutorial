use core::fmt::Write;

use arm_pl011_uart::Uart;
use log::{LevelFilter, Log, max_level, set_logger, set_max_level};
use spin::Mutex;
use spin::Once;

static UART: Once<Mutex<Uart>> = Once::new();

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        if let Some(uart) = UART.get() {
            let mut uart = uart.lock();
            writeln!(uart, "{}", record.args()).unwrap();
        } else {
            #[cfg(feature = "semihosting")]
            semihosting::println!("{}", record.args());
        };
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

pub fn init() {
    set_logger(&LOGGER).unwrap();
    set_max_level(LevelFilter::Trace);
}

pub fn init_uart(uart: Uart<'static>) {
    UART.call_once(|| Mutex::new(uart));
}
