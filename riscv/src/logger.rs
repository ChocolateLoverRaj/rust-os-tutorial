use core::fmt::Write;

use log::max_level;
use spin::{Mutex, Once};

use crate::sbi::Console;

struct Logger {
    console: Mutex<Console>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &log::Record) {
        let mut console = self.console.try_lock().unwrap();
        writeln!(console, "{}", record.args()).unwrap();
    }

    fn flush(&self) {}
}
static LOGGER: Once<Logger> = Once::new();

/// # Safety
/// Only call this function once
pub unsafe fn init() {
    let logger = LOGGER.call_once(|| Logger {
        console: Mutex::new(Console::take().unwrap()),
    });
    // Safety: nothing else is calling this function
    unsafe { log::set_logger_racy(logger).unwrap() };
    // Safety: nothing else is calling this function
    unsafe { log::set_max_level_racy(log::LevelFilter::Trace) };
}
