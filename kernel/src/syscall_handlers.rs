use core::arch::asm;

use alloc::collections::btree_map::{self, BTreeMap};
use common::Syscall;
use exists::SyscallExistsHandler;
use exit::SyscallExitHandler;
use log::SyscallLogHandler;
use mem_info::SyscallMemInfoHandler;
use syscall_alloc::SyscallAllocHandler;
use syscall_alloc_2::SyscallAlloc2Handler;

use crate::cpu_local_data::get_local;

mod exists;
mod exit;
mod log;
mod mem_info;
mod syscall_alloc;
mod syscall_alloc_2;

struct ExtraData<'a> {
    rflags: u64,
    return_instruction_pointer: u64,
    syscall_handlers: &'a SyscallHandlers,
}

pub struct SyscallHelper<'a, S: Syscall> {
    input: S::Input,
    extra_data: ExtraData<'a>,
}

impl<S: Syscall> SyscallHelper<'_, S> {
    pub fn input(&self) -> &S::Input {
        &self.input
    }

    pub fn syscall_return(&self, output: &S::Output) -> ! {
        let output = S::encode_output(output);

        let user_mode_stack_pointer_ptr = get_local().user_mode_stack_pointer.get();
        // Safety: the stack pointer was saved by the raw_syscall_handler
        let user_mode_stack_pointer = unsafe { user_mode_stack_pointer_ptr.read() };
        unsafe {
            asm!(
                "
                mov rsp, {}
                sysretq
            ",
                in(reg) user_mode_stack_pointer,
                in("rcx") self.extra_data.return_instruction_pointer,
                in("r11") self.extra_data.rflags,
                in("rdi") output[0],
                in("rsi") output[1],
                in("rdx") output[2],
                in("r10") output[3],
                in("r8") output[4],
                in("r9") output[5],
                in("rax") output[6],
                options(noreturn)
            );
        }
    }

    pub fn handler_exists(&self, id: &u64) -> bool {
        self.extra_data.syscall_handlers.map.contains_key(id)
    }
}

pub trait GenericSyscallHandler: Sync {
    type S: Syscall;

    fn handle_decoded_syscall(helper: SyscallHelper<Self::S>) -> !;
}

trait SyscallHandler: Sync {
    fn id(&self) -> u64;
    fn handle_syscall(&self, input: [u64; 6], extra_data: ExtraData) -> !;
}

impl<T: GenericSyscallHandler> SyscallHandler for T {
    fn id(&self) -> u64 {
        T::S::ID
    }
    fn handle_syscall(&self, input: [u64; 6], extra_data: ExtraData) -> ! {
        match T::S::try_decode_input(&input) {
            Ok(input) => T::handle_decoded_syscall(SyscallHelper { input, extra_data }),
            Err(e) => {
                let id = T::S::ID;
                todo!(
                    "Invalid syscall input for 0x{id:X}. Error: {e}. Input: {input:?}. Terminate process",
                );
            }
        }
    }
}

static SYSCALL_HANDLERS: &[&dyn SyscallHandler] = &[
    &SyscallExistsHandler,
    &SyscallExitHandler,
    &SyscallLogHandler,
    &SyscallMemInfoHandler,
    &SyscallAllocHandler,
    &SyscallAlloc2Handler,
];
pub struct SyscallHandlers {
    map: BTreeMap<u64, &'static dyn SyscallHandler>,
}
impl Default for SyscallHandlers {
    fn default() -> Self {
        Self {
            map: {
                let mut map = BTreeMap::default();
                for &syscall_handler in SYSCALL_HANDLERS {
                    match map.entry(syscall_handler.id()) {
                        btree_map::Entry::Vacant(entry) => entry.insert(syscall_handler),
                        btree_map::Entry::Occupied(entry) => {
                            panic!("Duplicate syscall handler: {}", entry.key());
                        }
                    };
                }
                map
            },
        }
    }
}
impl SyscallHandlers {
    #[allow(clippy::too_many_arguments)]
    pub fn handle_syscall(
        &self,
        input0: u64,
        input1: u64,
        input2: u64,
        input3: u64,
        input4: u64,
        input5: u64,
        input6: u64,
        rflags: u64,
        return_instruction_pointer: u64,
    ) -> ! {
        let id = input0;
        let input = [input1, input2, input3, input4, input5, input6];
        match self.map.get(&id) {
            Some(handler) => handler.handle_syscall(
                input,
                ExtraData {
                    rflags,
                    return_instruction_pointer,
                    syscall_handlers: self,
                },
            ),
            None => todo!("Invalid syscall id: {id}. Terminate process"),
        }
    }
}
