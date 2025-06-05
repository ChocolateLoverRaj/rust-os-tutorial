use core::slice;

use common::{Syscall, SyscallLog, SyscallLogError};
use console::strip_ansi_codes;
use nodit::Interval;

use crate::{logger::log_for_user_mode, run_user_mode_program::TASK};

use super::GenericSyscallHandler;

pub struct SyscallLogHandler;
impl GenericSyscallHandler for SyscallLogHandler {
    type S = SyscallLog;
    fn handle_decoded_syscall(input: super::SyscallInput<Self::S>) -> ! {
        enum Action {
            Return(<SyscallLog as Syscall>::Output),
            Terminate,
        }
        match {
            let actual_input = input.input();
            if actual_input.message.len() > 0 {
                let task = TASK.lock();
                let task = task.as_ref().unwrap();
                let start = actual_input.message.pointer();
                let len = actual_input.message.len();
                let end_inclusive = start + (len - 1);
                if task
                    .mapped_virtual_memory
                    .contains_interval(Interval::from(start..=end_inclusive))
                {
                    // Safety: the message is mapped in the lower half
                    let message =
                        unsafe { slice::from_raw_parts(start as *const u8, len as usize) };
                    Action::Return(if let Ok(message) = str::from_utf8(message) {
                        log_for_user_mode(actual_input.level, {
                            // Don't let user mode code print colors and possibly mess up terminal cursor position
                            strip_ansi_codes(message)
                        });
                        Ok(())
                    } else {
                        Err(SyscallLogError::InvalidString)
                    })
                } else {
                    Action::Terminate
                }
            } else {
                Action::Return(Ok(()))
            }
        } {
            Action::Return(output) => input.syscall_return(&output),
            Action::Terminate => {
                todo!("Invalid memory. Terminate process")
            }
        }
    }
}
