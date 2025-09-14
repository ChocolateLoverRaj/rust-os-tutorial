use abi::HELLO_WORLD_MAGIC;

use super::data::SyscallData;

pub fn s_hello_world(data: SyscallData) -> ! {
    let input = data.input;
    if input == HELLO_WORLD_MAGIC {
        log::debug!("User mode program succesfully did a hello world syscall.");
    } else {
        log::debug!(
            "User mode program did a syscall with the hello world syscall number, but the input 0x{input:X}, which is not the expected input: 0x{HELLO_WORLD_MAGIC:X}."
        );
    };
    data.ret()
}
