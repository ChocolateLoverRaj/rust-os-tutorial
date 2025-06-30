use alloc::string::ToString;
use common::log::{self, LevelFilter, Log};

use crate::syscalls::syscall_log;

struct Logger;

impl Log for Logger {
    fn enabled(&self, _metadata: &common::log::Metadata) -> bool {
        todo!()
    }

    fn log(&self, record: &common::log::Record) {
        syscall_log(record.level(), &record.args().to_string());
    }

    fn flush(&self) {
        todo!()
    }
}

static LOGGER: Logger = Logger;

pub fn init() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::max());
}
