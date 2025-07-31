use core::arch::asm;

use raw_cpuid::CpuId;
use x86_64::registers::control::{Cr4, Cr4Flags};

fn has_smep() -> bool {
    if let Some(extended_features) = CpuId::new().get_extended_feature_info() {
        extended_features.has_smep()
    } else {
        false
    }
}

pub fn has_smap() -> bool {
    if let Some(extended_features) = CpuId::new().get_extended_feature_info() {
        extended_features.has_smap()
    } else {
        false
    }
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

pub fn init() {
    let mut flags = Cr4::read();
    if has_smep() {
        flags |= Cr4Flags::SUPERVISOR_MODE_EXECUTION_PROTECTION;
    }
    if has_smap() {
        flags |= Cr4Flags::SUPERVISOR_MODE_ACCESS_PREVENTION;
    }
    // Safety: the flags we enable don't cause any safety violations
    unsafe { Cr4::write(flags) };
}
