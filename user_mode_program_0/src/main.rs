#![no_std]
#![no_main]

use core::{arch::asm, hint::black_box, mem::MaybeUninit, ptr::NonNull, sync::atomic::AtomicU8};

use abi::{
    HELLO_WORLD_MAGIC, Slice, SyscallError, SyscallLog, SyscallLogOutput, SyscallNumber,
    decode_syscall_output,
};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

static TEST_VAR: AtomicU8 = AtomicU8::new(0);

/// # Safety
/// Syscalls can read / write to memory. You must do this safely.
unsafe fn syscall(syscall_number: SyscallNumber, input: usize) -> Result<(), SyscallError> {
    let syscall_number = u32::from(syscall_number);
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

fn syscall_hello_world() {
    // Safety: no memory is modified
    unsafe { syscall(SyscallNumber::HelloWorld, HELLO_WORLD_MAGIC) }.unwrap();
}

fn syscall_log(message: &str) {
    let mut data = SyscallLog {
        slice: Slice {
            addr: message.as_ptr().addr(),
            len: message.len(),
        },
        output: MaybeUninit::uninit(),
    };
    // Safety: slice is valid
    unsafe {
        syscall(
            SyscallNumber::Log,
            NonNull::from_mut(&mut data).addr().get(),
        )
    }
    .unwrap();
    // Safety: the output has been initialized
    let output = unsafe { data.output.assume_init() };
    if !matches!(output, SyscallLogOutput::Ok) {
        unreachable!("{output:?}")
    }
}

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    black_box(&TEST_VAR);
    syscall_hello_world();
    syscall_log("Hello from user mode program 🚀");
    loop {
        core::hint::spin_loop();
    }
}
