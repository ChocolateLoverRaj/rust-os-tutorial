#![no_std]
#![no_main]

use core::arch::asm;

use common::{Syscall, SyscallExists, SyscallExit, SyscallLog, SyscallLogInput, log};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    syscall_log(log::Level::Error, "panicked");
    syscall_exit()
}

/// # Safety
/// The input must be valid. Invalid inputs can lead to undefined behavior or the program being terminated.
unsafe fn raw_syscall(input_and_ouput: &mut [u64; 7]) {
    unsafe {
        asm!(
            "syscall",
            inlateout("rdi") input_and_ouput[0],
            inlateout("rsi") input_and_ouput[1],
            inlateout("rdx") input_and_ouput[2],
            inlateout("r10") input_and_ouput[3],
            inlateout("r8") input_and_ouput[4],
            inlateout("r9") input_and_ouput[5],
            inlateout("rax") input_and_ouput[6],
        );
    }
}

/// # Safety
/// Input must be valid, and the kernel should support the syscall
unsafe fn syscall<T: Syscall>(input: &T::Input) -> T::Output {
    let mut inputs_and_ouputs = T::encode_input(input);
    unsafe { raw_syscall(&mut inputs_and_ouputs) };
    T::decode_output(&inputs_and_ouputs)
}

fn syscall_exists(syscall_id: u64) -> bool {
    // Safety: there is nothing that can go wrong with this syscall
    unsafe { syscall::<SyscallExists>(&syscall_id) }
}

fn syscall_exit() -> ! {
    // Safety: input is valid
    unsafe { syscall::<SyscallExit>(&()) };
    unreachable!()
}

fn syscall_log(level: log::Level, message: &str) {
    unsafe {
        syscall::<SyscallLog>(&SyscallLogInput {
            level,
            message: message.as_bytes().into(),
        })
    }
    // &str means it should be valid
    .unwrap()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    let should_be_true = syscall_exists(SyscallExit::ID);
    let should_be_false = syscall_exists(0);
    assert!(should_be_true);
    assert!(!should_be_false);
    syscall_log(log::Level::Info, "Hello from user mode program 🚀");
    panic!("test panick")
}
