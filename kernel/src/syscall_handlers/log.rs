use core::{cell::RefCell, fmt::Display, slice};

use alloc::boxed::Box;
use common::{LOWER_HALF_END, SliceData, SyscallLog, SyscallLogError};
use console::WithoutAnsi;
use itertools::Itertools;

use crate::{
    cpu_local_data::get_local, logger::log_for_user_mode, try_access_user_mem::try_access_user_mem,
};

use super::GenericSyscallHandler;

pub struct SyscallLogHandler;
impl GenericSyscallHandler for SyscallLogHandler {
    type S = SyscallLog;
    fn handle_decoded_syscall(input: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let input = input.input();

            if input.message.pointer() == 0
                || input.message.pointer() + input.message.len() > LOWER_HALF_END
            {
                Err(SyscallLogError::InvalidPointer)?
            }

            let local = get_local();
            let thread_id = local.running_thread.lock().unwrap();

            let error = Default::default();
            struct D<'a> {
                slice_data: SliceData,
                error: &'a RefCell<Option<SyscallLogError>>,
            }
            impl Display for D<'_> {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    let mut buffer = [Default::default(); 0x1000];
                    let mut position = 0;
                    let mut buffer_used_len = 0;
                    loop {
                        let data = (self.slice_data.pointer() as usize + position) as *const u8;
                        let len = (self.slice_data.len() as usize - position)
                            .min(buffer.len() - buffer_used_len);
                        if len == 0 {
                            break;
                        }
                        let ptr: *const [u8] = unsafe { slice::from_raw_parts(data, len) };
                        if let Err(_e) = try_access_user_mem(|| {
                            let src = unsafe { ptr.as_ref() }.unwrap();
                            buffer[buffer_used_len..buffer_used_len + len].copy_from_slice(src);
                            Box::new(())
                        }) {
                            *self.error.borrow_mut() = Some(SyscallLogError::InvalidPointer);
                            break;
                        }
                        position += len;
                        match buffer[..buffer_used_len + len].utf8_chunks().exactly_one() {
                            Ok(chunk) => {
                                if !chunk.invalid().is_empty()
                                    && position == self.slice_data.len() as usize
                                {
                                    // This means that the message ends with invalid UTF-8
                                    *self.error.borrow_mut() = Some(SyscallLogError::InvalidString);
                                    break;
                                }
                                // Remove ANSI from the string because we don't want user mode messing up the terminal
                                WithoutAnsi::new(chunk.valid()).fmt(f)?;
                                // If the message got split at a UTF-8 character boundary, we will have up to 3 invalid chars
                                let invalid_start = chunk.valid().len();
                                let invalid_len = chunk.invalid().len();
                                buffer.copy_within(invalid_start..invalid_start + invalid_len, 0);
                                buffer_used_len = invalid_len;
                            }
                            Err(_) => {
                                // There was invalid UTF-8 in the middle of the message, so overall the message is not valid UTF-8
                                *self.error.borrow_mut() = Some(SyscallLogError::InvalidString);
                                break;
                            }
                        }
                    }
                    Ok(())
                }
            }
            log_for_user_mode(
                input.level,
                D {
                    slice_data: input.message,
                    error: &error,
                },
                thread_id,
            );

            if let Some(error) = error.into_inner() {
                Err(error)?
            }
            Ok(())
        })();
        input.syscall_return(&output)
    }
}
