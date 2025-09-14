use core::arch::asm;

use abi::{SyscallError, encode_syscall_output};

pub struct SyscallData {
    pub input: usize,
    pub return_stack_pointer: u64,
    pub return_instruction_pointer: u64,
    pub rflags: u64,
}

impl SyscallData {
    fn sysretq_result(self, output: Result<(), SyscallError>) -> ! {
        let output = encode_syscall_output(output);
        unsafe {
            asm!(
                "
                mov rsp, {}
                sysretq
            ",
                in(reg) self.return_stack_pointer,
                in("rcx") self.return_instruction_pointer,
                in("r11") self.rflags,
                in("rdi") 0,
                in("rsi") 0,
                in("rdx") 0,
                in("r10") 0,
                in("r8") 0,
                in("r9") 0,
                in("rax") output,
                options(noreturn)
            );
        }
    }

    pub fn ret(self) -> ! {
        self.sysretq_result(Ok(()))
    }

    pub fn ret_no_exist(self) -> ! {
        self.sysretq_result(Err(SyscallError::SyscallNoExist))
    }
}
