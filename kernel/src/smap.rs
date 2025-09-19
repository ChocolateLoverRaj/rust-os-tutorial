use core::arch::asm;

use raw_cpuid::CpuId;

pub fn has_smap() -> bool {
    CpuId::new()
        .get_extended_feature_info()
        .is_some_and(|info| info.has_smap())
}

/// https://www.felixcloutier.com/x86/stac
///
///  Sets the AC flag bit in EFLAGS register.
/// This may enable alignment checking of user-mode data accesses.
/// This allows explicit supervisor-mode data accesses to user-mode pages even if the SMAP bit is set in the CR4 register.
pub fn stac() {
    unsafe {
        asm!("stac", options(nomem, nostack));
    }
}

/// https://www.felixcloutier.com/x86/clac
///
/// Clears the AC flag bit in EFLAGS register.
/// This disables any alignment checking of user-mode data accesses.
/// If the SMAP bit is set in the CR4 register, this disallows explicit supervisor-mode data accesses to user-mode pages.
pub fn clac() {
    unsafe {
        asm!("clac", options(nomem, nostack));
    }
}
