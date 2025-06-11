use alloc::collections::btree_map::{self, BTreeMap};
use common::Syscall;
use exists::SyscallExistsHandler;
use exit::SyscallExitHandler;
use frame_buffer::{SyscallReleaseFrameBufferHandler, SyscallTakeFrameBufferHandler};
use keyboard::{SyscallReadKeyboardHandler, SyscallSubscribeToKeyboardHandler};
use log::SyscallLogHandler;
use syscall_alloc::SyscallAllocHandler;
use wait_until_event::SyscallWaitUntilEventHandler;

use crate::syscall_saved_regs::SyscallSavedRegs;

mod exists;
mod exit;
mod frame_buffer;
mod keyboard;
mod log;
mod mouse;
mod syscall_alloc;
mod wait_until_event;

struct ExtraData<'a> {
    saved_regs: &'a mut SyscallSavedRegs,
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
        unsafe { self.extra_data.saved_regs.sysretq(output) }
    }

    pub fn handler_exists(&self, id: &u64) -> bool {
        self.extra_data.syscall_handlers.map.contains_key(id)
    }

    pub fn saved_regs(&self) -> &SyscallSavedRegs {
        self.extra_data.saved_regs
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
    &SyscallAllocHandler,
    &SyscallTakeFrameBufferHandler,
    &SyscallReleaseFrameBufferHandler,
    &SyscallSubscribeToKeyboardHandler,
    &SyscallReadKeyboardHandler,
    &SyscallWaitUntilEventHandler,
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
        syscall_saved_regs: &mut SyscallSavedRegs,
    ) -> ! {
        let id = input0;
        let input = [input1, input2, input3, input4, input5, input6];
        match self.map.get(&id) {
            Some(handler) => handler.handle_syscall(
                input,
                ExtraData {
                    saved_regs: syscall_saved_regs,
                    syscall_handlers: self,
                },
            ),
            None => todo!("Invalid syscall id: {id}. Terminate process"),
        }
    }
}
