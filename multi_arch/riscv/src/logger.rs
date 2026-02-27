use core::fmt::{Arguments, Write};

use kernel_lib::EarlyLogger;

pub struct SbiEarlyLogger;

// impl EarlyLogger for SbiEarlyLogger {}

impl Write for SbiEarlyLogger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &byte in s.as_bytes() {
            sbi::legacy::console_putchar(byte);
        }
        Ok(())
    }
}

pub static EARLY_LOGGER: SbiEarlyLogger = SbiEarlyLogger;

pub fn early_log(arguments: Arguments<'_>) {
    pub struct SbiEarlyLogger;

    impl Write for SbiEarlyLogger {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &byte in s.as_bytes() {
                sbi::legacy::console_putchar(byte);
            }
            Ok(())
        }
    }

    writeln!(SbiEarlyLogger, "{arguments}");
}
