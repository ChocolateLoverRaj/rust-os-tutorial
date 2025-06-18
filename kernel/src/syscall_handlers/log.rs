use core::slice;

use common::{Syscall, SyscallLog, SyscallLogError};
use console::strip_ansi_codes;
use nodit::Interval;

use crate::{cpu_local_data::get_local, logger::log_for_user_mode, task::THREADS};

use super::GenericSyscallHandler;

pub struct SyscallLogHandler;
impl GenericSyscallHandler for SyscallLogHandler {
    type S = SyscallLog;
    fn handle_decoded_syscall(input: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return(<SyscallLog as Syscall>::Output),
            Terminate,
        }
        let action = {
            let actual_input = input.input();
            if actual_input.message.len() > 0 {
                let threads = THREADS.read();
                let local = get_local();
                let current_process = &threads
                    .get(&local.running_thread.lock().unwrap())
                    .unwrap()
                    .process;
                let start = actual_input.message.pointer();
                let len = actual_input.message.len();
                let end_inclusive = start + (len - 1);
                if current_process
                    .mapped_virtual_memory
                    .read()
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
        };
        match action {
            Action::Return(output) => input.syscall_return(&output),
            Action::Terminate => {
                todo!("Invalid memory. Terminate process")
            }
        }
    }
}
