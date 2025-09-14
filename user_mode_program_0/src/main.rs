#![no_std]
#![no_main]

use core::{arch::asm, hint::black_box, sync::atomic::AtomicU8};

use abi::{HELLO_WORLD_MAGIC, Syscall, SyscallError, decode_syscall_output};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

static TEST_VAR: AtomicU8 = AtomicU8::new(0);

fn syscall(syscall_number: u32, input: usize) -> Result<(), SyscallError> {
    let output: u32;
    unsafe {
        asm!("\
            syscall
            ",
            in("rdi") syscall_number,
            in("rsi") input,
            lateout("rax") output,
            clobber_abi("sysv64")
        );
    };
    decode_syscall_output(output).unwrap()
}

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    black_box(&TEST_VAR);
    syscall(Syscall::HelloWorld.into(), HELLO_WORLD_MAGIC).unwrap();
    syscall(Syscall::HelloWorld.into(), 0).unwrap();
    loop {
        core::hint::spin_loop();
    }
}
