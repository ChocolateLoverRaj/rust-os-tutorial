use core::{
    cell::RefCell,
    fmt::Display,
    mem::MaybeUninit,
    ptr::{NonNull, slice_from_raw_parts, slice_from_raw_parts_mut},
};

use abi::{Slice, SyscallLog, SyscallLogOutput};
use console::WithoutAnsi;
use itertools::Itertools;

use crate::{logger::log_for_user_mode, *};

use super::*;

enum UserPointerError {
    /// The value of this pointer is `0x0`.
    /// Technically the machine code can use `0x0` as a valid address, but Rust panics if you try to.
    Null,
    /// The pointer is not within the lower half of memory
    NotWithinLowerHalf,
}

fn validate_user_ptr<T>(addr: usize) -> Result<NonNull<T>, UserPointerError> {
    let ptr_end = addr
        .checked_add(size_of::<T>())
        .ok_or(UserPointerError::NotWithinLowerHalf)?;
    if ptr_end <= LOWER_HALF_END as usize {
        Ok(NonNull::new(addr as *mut T).ok_or(UserPointerError::Null)?)
    } else {
        Err(UserPointerError::NotWithinLowerHalf)
    }
}

fn validate_user_slice<T>(slice: Slice) -> Result<NonNull<[T]>, UserPointerError> {
    let ptr_end = slice
        .addr
        .checked_add(
            size_of::<T>()
                .checked_mul(slice.len)
                .ok_or(UserPointerError::NotWithinLowerHalf)?,
        )
        .ok_or(UserPointerError::NotWithinLowerHalf)?;
    if ptr_end <= LOWER_HALF_END as usize {
        Ok(
            NonNull::new(slice_from_raw_parts_mut(slice.addr as *mut T, slice.len))
                .ok_or(UserPointerError::Null)?,
        )
    } else {
        Err(UserPointerError::NotWithinLowerHalf)
    }
}

pub fn s_log(data: SyscallData) -> ! {
    if let Ok(mut input_ptr) = validate_user_ptr::<SyscallLog>(data.input) {
        let mut input = MaybeUninit::<SyscallLog>::uninit();
        if try_access_user_mem(|| {
            input.write({
                // Safety: we handle the page fault
                unsafe { input_ptr.read() }
            });
        })
        .is_err()
        {
            data.ret_invalid_input();
        }
        // Safety: the closure executed successfully, so it initialized the input
        let input = unsafe { input.assume_init() };
        let output = if let Ok(ptr) = validate_user_slice::<u8>(input.slice) {
            let output = RefCell::new(SyscallLogOutput::Ok);
            // Note that we currently have no limit on the length of the user message
            // This is great if we want to dump large amounts of debug information during development
            // However, in production large messages could effectively block the OS from doing anything else while the message is logging
            // So in production we may want to do one of the following:
            // - set a maximum message length
            // - disable logging directly to the serial port and screen, and save the log in memory or in a file
            log_for_user_mode(ChunkedCopyStr {
                ptr,
                error: &output,
            });
            output.into_inner()
        } else {
            SyscallLogOutput::InvalidSlice
        };
        if try_access_user_mem(|| {
            unsafe { input_ptr.as_mut() }.output.write(output);
        })
        .is_err()
        {
            data.ret_invalid_input()
        };
        data.ret_ok()
    } else {
        data.ret_invalid_input()
    }
}

/// Because the message to log can be very large, we can't allocate a fixed amount of memory to store the message on the kernel's stack.
/// We will not be creating dynamic allocations in the kernel to copy the message because that would unnecessarilly use a lot of memory.
/// Instead, we will copy the string in chunks, validating the string in the process.
struct ChunkedCopyStr<'a> {
    /// The user message
    ptr: NonNull<[u8]>,
    /// If there is an error, we need some way of storing it.
    /// Since [`Display::fmt`] only gives an immutable ref, we will use [`RefCell`].
    error: &'a RefCell<SyscallLogOutput>,
}
impl Display for ChunkedCopyStr<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut error = self.error.borrow_mut();
        // The buffer is used in each iteration of the loop
        // Note that the buffer size must be at least 4, because a single valid UTF-8 character can be up to 4 bytes
        let mut buffer = [Default::default(); 0x1000];
        let mut position = 0;
        // If there are bytes leading to an incomplete UTF-8 character in the end, the are shifted all the way to the left in the buffer
        let mut buffer_used_len = 0;
        loop {
            let data = (self.ptr.addr().get() + position) as *mut u8;
            let len = (self.ptr.len() - position).min(buffer.len() - buffer_used_len);
            if len == 0 {
                break;
            }
            let ptr = slice_from_raw_parts(data, len);
            // Copy the chunk from user mem to kernel mem
            if let Err(_e) = try_access_user_mem(|| {
                let src = unsafe { ptr.as_ref() }.unwrap();
                buffer[buffer_used_len..buffer_used_len + len].copy_from_slice(src);
            }) {
                *error = SyscallLogOutput::InvalidSlice;
                break;
            }
            position += len;
            // A valid message should have exactly 1 valid UTF-8 chunk
            match buffer[..buffer_used_len + len].utf8_chunks().exactly_one() {
                Ok(chunk) => {
                    // If the *chunk* ends with invalid UTF-8, the *message* could still be valid
                    // In this case, a multi-byte UTF-8 character got split at the chunk boundary
                    // However, if the *message* ends with invalid UTF-8, the message overall is invalid
                    if !chunk.invalid().is_empty() && position == self.ptr.len() {
                        // This means that the message ends with invalid UTF-8
                        *error = SyscallLogOutput::InvalidUtf8;
                        break;
                    }
                    // Remove ANSI from the string because we don't want user mode messing up the terminal
                    WithoutAnsi::new(chunk.valid()).fmt(f)?;
                    // If the message got split at a UTF-8 character boundary, we will have up to 3 invalid chars
                    let invalid_start = chunk.valid().len();
                    let invalid_len = chunk.invalid().len();
                    // Shift the up to 3 invalid chars to the start of the buffer, so that we can reconsider those bytes in the next chunk
                    buffer.copy_within(invalid_start..invalid_start + invalid_len, 0);
                    buffer_used_len = invalid_len;
                }
                Err(_) => {
                    // If there are 0 valid UTF-8 chunks, that means that the message is not valid UTF-8
                    // If there are 2+ valid UTF-8 chunks, that means there was invalid UTF-8 in the middle of the message, so overall the message is not valid UTF-8
                    *error = SyscallLogOutput::InvalidUtf8;
                    break;
                }
            }
        }
        Ok(())
    }
}
