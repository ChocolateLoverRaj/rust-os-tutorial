#[cfg(feature = "defmt")]
mod defmt_logger {
    use defmt::global_logger;

    #[global_logger]
    struct DefmtLogger;
    unsafe impl defmt::Logger for DefmtLogger {
        fn acquire() {
            // todo!()
        }

        unsafe fn flush() {
            // todo!()
        }

        unsafe fn release() {
            // todo!()
        }

        unsafe fn write(bytes: &[u8]) {
            for byte in bytes {
                sbi::legacy::console_putchar(*byte);
            }
        }
    }
}

#[cfg(feature = "log")]
mod log_logger {
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
}

/// # Safety
/// Only call once.
pub unsafe fn init() {
    #[cfg(feature = "defmt")]
    sbi::legacy::console_putchar(0);

    #[cfg(feature = "log")]
    {
        // Safety: nothing else is calling this function
        unsafe { log::set_logger_racy(&log_logger::LOGGER).unwrap() };
        // Safety: nothing else is calling this function
        unsafe { log::set_max_level_racy(log::LevelFilter::Trace) };
    }
}
