#[cfg(feature = "defmt")]
mod defmt_logger {
    use core::mem;

    use defmt::{Encoder, global_logger};
    use spin::Mutex;

    static ENCODER: Mutex<Encoder> = Mutex::new(Encoder::new());

    fn write(bytes: &[u8]) {
        for byte in bytes {
            sbi::legacy::console_putchar(*byte);
        }
    }

    #[global_logger]
    struct DefmtLogger;
    unsafe impl defmt::Logger for DefmtLogger {
        fn acquire() {
            let mut t = ENCODER.lock();
            t.start_frame(write);
            mem::forget(t);
        }

        unsafe fn flush() {
            // No need to do anything
        }

        unsafe fn release() {
            unsafe { ENCODER.force_unlock() };
            let mut encoder = ENCODER.lock();
            encoder.end_frame(write);
        }

        unsafe fn write(bytes: &[u8]) {
            unsafe { ENCODER.force_unlock() };
            let mut encoder = ENCODER.lock();
            encoder.write(bytes, write);
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
