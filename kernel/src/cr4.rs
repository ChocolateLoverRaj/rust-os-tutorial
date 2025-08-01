use raw_cpuid::CpuId;
use x86_64::registers::control::{Cr4, Cr4Flags};

use crate::smep_smap;

/// Initialize things related to the `Cr4` register. This function should be called on all CPUs.
pub fn init() {
    smep_smap::init();
}

pub fn init_pge() {
    if CpuId::new()
        .get_feature_info()
        .is_some_and(|feature_info| feature_info.has_pge())
    {
        unsafe {
            Cr4::update(|flags| {
                *flags |= Cr4Flags::PAGE_GLOBAL;
            })
        };
    }
}
