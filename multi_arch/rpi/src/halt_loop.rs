pub fn halt_loop() -> ! {
    loop {
        #[cfg(target_arch = "arm")]
        unsafe {
            core::arch::arm::__wfe();
        };
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::aarch64::__wfe();
        }
    }
}
