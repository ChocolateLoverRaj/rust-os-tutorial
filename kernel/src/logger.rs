use core::fmt::{Display, Write};

use log::{LevelFilter, Log};
use owo_colors::OwoColorize;
use spinning_top::Spinlock;
use uart_16550::SerialPort;

use crate::cpu_local_data::try_get_local;

struct KernelLogger {
    serial_port: Spinlock<SerialPort>,
}

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        todo!()
    }

    fn log(&self, record: &log::Record) {
        let mut serial_port = self.serial_port.lock();
        let level = record.level();
        let level: &dyn Display = match level {
            log::Level::Error => &level.bright_red(),
            log::Level::Warn => &level.bright_yellow(),
            log::Level::Info => &level.bright_blue(),
            log::Level::Debug => &level.bright_cyan(),
            log::Level::Trace => &level.bright_magenta(),
        };
        if let Some(cpu_local_data) = try_get_local() {
            let cpu_id = cpu_local_data.cpu.id;
            write!(serial_port, "{}", format_args!("[CPU {}]", cpu_id).dimmed()).unwrap();
        } else {
            write!(serial_port, "{}", "[BSP]".dimmed()).unwrap();
        }
        let args = record.args();
        writeln!(serial_port, " {:5} {}", level, args).unwrap();
    }

    fn flush(&self) {
        todo!()
    }
}

static LOGGER: KernelLogger = KernelLogger {
    serial_port: Spinlock::new(unsafe { SerialPort::new(0x3F8) }),
};

pub fn init() -> Result<(), log::SetLoggerError> {
    LOGGER.serial_port.try_lock().unwrap().init();
    log::set_max_level(LevelFilter::max());
    log::set_logger(&LOGGER)
}
