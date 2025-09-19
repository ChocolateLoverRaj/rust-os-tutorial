use core::{arch::naked_asm, mem::offset_of, ptr::NonNull};

use alloc::boxed::Box;
use x86_64::VirtAddr;

use crate::{
    cpu_local_data::{CpuLocalData, get_local},
    smap::{clac, has_smap, stac},
};

/// In the future we could put more info from the page fault handler into here
#[derive(Debug)]
pub struct AccessUserMemError {
    #[allow(unused)]
    pub accessed_address: VirtAddr,
}

/// # Safety
/// The pointer must be a valid pointer to a boxed closure that was created with [`Box::into_raw`]
unsafe extern "sysv64" fn call_inside_try_access_user_mem(f: NonNull<Box<dyn FnOnce()>>) {
    let raw = f.as_ptr();
    // Safety: the pointer was created using Box::into_raw
    let f = unsafe { Box::from_raw(raw) };
    f();
}

/// We double box the closure so that we can turn it into a single u64 pointer
#[unsafe(naked)]
unsafe extern "sysv64" fn _try_access_user_mem<'a>(f: *mut Box<dyn FnOnce() + 'a>) {
    naked_asm!(
        "
            // Back up callee-saved registers, because they will not be restored properly by the closure if a page fault happens
            push rbx
            push rbp
            push r12
            push r13
            push r14
            push r15

            // Save the stack pointer so that the page fault handler can return to this assembly code if a page fault happens 
            mov gs:[{try_access_user_mem_rsp_offset}], rsp

            // The arg will already be in rdi, since we didn't modify them
            call {call_closure}

            // Restore callee-saved registers
            pop r15
            pop r14
            pop r13
            pop r12
            pop rbp
            pop rbx

            // Return
            ret
        ",
        try_access_user_mem_rsp_offset = const offset_of!(CpuLocalData, try_access_user_mem_rsp),
        call_closure = sym call_inside_try_access_user_mem,
    )
}

/// This works as a `copy_from_user` and `copy_to_user`, except that you do the copying in your own closure!
/// Remember that if at any point inside the closure a page fault happens, the closure will stop executing and this function will return an error.
/// So don't own any mutexes or anything that implements `Drop` inside the closure.
pub fn try_access_user_mem<'a>(f: impl FnOnce() + 'a) -> Result<(), AccessUserMemError> {
    let f = Box::into_raw(Box::new(Box::new(f) as Box<dyn FnOnce() + 'a>));
    let local = get_local();
    // Set the result to Ok. If a page fault happens, it will be changed to Err
    *local.try_access_user_mem_result.try_lock().unwrap() = Some(Ok(()));
    if has_smap() {
        stac();
    }
    // Safety: the pointer is a boxed closure
    unsafe { _try_access_user_mem(f) };
    if has_smap() {
        clac();
    }
    // Retrieve the result
    local
        .try_access_user_mem_result
        .try_lock()
        .unwrap()
        .take()
        .unwrap()
}
