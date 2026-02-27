use spin::Once;

use crate::EarlyLogger;

pub struct Arch {
    pub early_log: EarlyLogger,
    pub shutdown: Option<fn() -> !>,
    pub low_power_loop: fn() -> !,
}

pub static ARCH: Once<Arch> = Once::new();

pub fn arch() -> &'static Arch {
    ARCH.get().unwrap()
}
