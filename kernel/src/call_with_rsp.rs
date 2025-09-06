use core::arch::naked_asm;

/// # Safety
/// Stack must be valid
#[unsafe(naked)]
pub unsafe extern "sysv64" fn call_with_rsp(new_rsp: u64, f: extern "sysv64" fn() -> !) -> ! {
    naked_asm!(
        "
        mov rsp, rdi
        call rsi
        "
    );
}
