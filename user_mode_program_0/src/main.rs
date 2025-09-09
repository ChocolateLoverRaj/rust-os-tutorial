#![no_std]
#![no_main]

use core::{arch::asm, hint::black_box, sync::atomic::AtomicU8};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

static TEST_VAR: AtomicU8 = AtomicU8::new(0);

fn syscall(inputs_and_outputs: &mut [u64; 7]) {
    unsafe {
        asm!("\
            syscall
            ",
            inlateout("rdi") inputs_and_outputs[0],
            inlateout("rsi") inputs_and_outputs[1],
            inlateout("rdx") inputs_and_outputs[2],
            inlateout("r10") inputs_and_outputs[3],
            inlateout("r8") inputs_and_outputs[4],
            inlateout("r9") inputs_and_outputs[5],
            inlateout("rax") inputs_and_outputs[6],
        );
    }
}

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    black_box(&TEST_VAR);
    let mut inputs_and_outputs = [10, 20, 30, 40, 50, 60, 70];
    syscall(&mut inputs_and_outputs);
    syscall(&mut inputs_and_outputs);
    loop {
        core::hint::spin_loop();
    }
}
