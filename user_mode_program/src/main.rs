#![no_std]
#![no_main]

use core::arch::asm;

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// # Safety
/// The inputs must be valid. Invalid inputs can lead to undefined behavior or the program being terminated.
pub unsafe fn raw_syscall(inputs_and_ouputs: &mut [u64; 7]) {
    unsafe {
        asm!("\
            syscall
            ",
            inlateout("rdi") inputs_and_ouputs[0],
            inlateout("rsi") inputs_and_ouputs[1],
            inlateout("rdx") inputs_and_ouputs[2],
            inlateout("r10") inputs_and_ouputs[3],
            inlateout("r8") inputs_and_ouputs[4],
            inlateout("r9") inputs_and_ouputs[5],
            inlateout("rax") inputs_and_ouputs[6],
        );
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    let mut inputs_and_outputs = [10, 20, 30, 40, 50, 60, 70];
    unsafe { raw_syscall(&mut inputs_and_outputs) };
    loop {
        core::hint::spin_loop();
    }
}
