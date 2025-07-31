use core::{
    arch::naked_asm,
    mem::{MaybeUninit, offset_of},
    ptr::NonNull,
};

use alloc::boxed::Box;
use x86_64::VirtAddr;

use crate::{
    cpu_local_data::CpuLocalData,
    smep_smap::{clac, has_smap, stac},
};

/// In the future we could put more info from the page fault handler into here
#[derive(Debug)]
pub struct AccessUserMemError {
    pub accessed_address: VirtAddr,
}

unsafe extern "sysv64" fn call_inside_try_access_user_mem(
    f: *mut Box<dyn FnOnce() -> Box<()> + '_>,
) -> *mut () {
    let f = unsafe { Box::from_raw(f) };
    let r = f();
    Box::into_raw(r)
}

/// We double box the closure so that we can turn it into a single u64 pointer
#[unsafe(naked)]
unsafe extern "sysv64" fn _try_access_user_mem<T>(
    f: *mut Box<dyn FnOnce() -> Box<T> + '_>,
    e: &mut MaybeUninit<AccessUserMemError>,
) -> u64 {
    naked_asm!(
        "
            // Back up callee-saved registers, because they will not be restored properly by the closure if a page fault happens
            mov gs:[{copy_from_user_rbx}], rbx
            mov gs:[{copy_from_user_rbp}], rbp
            mov gs:[{copy_from_user_r12}], r12
            mov gs:[{copy_from_user_r13}], r13
            mov gs:[{copy_from_user_r14}], r14
            mov gs:[{copy_from_user_r15}], r15
            
            mov gs:[{copy_from_user_rsp}], rsp

            // Store the error pointer for the page fault handler to use if a page fault happens
            mov gs:[{access_user_mem_error_pointer}], rsi

            // Allow accessing user-accessible mem from kernel mode
            // stac
            // The arg will already be in rdi, since we didn't modify them
            call {call_closure}
            // Disable accessing user-accessible mem from kernel mode
            // clac

            // This indicates that we are no longer trying to access user mem
            mov qword ptr gs:[{access_user_mem_error_pointer}], 0

            // Keep the rax value as what the rust fn returned
            // Return
            ret
        ",
        copy_from_user_rbx = const offset_of!(CpuLocalData, copy_from_user_rbx),
        copy_from_user_rbp = const offset_of!(CpuLocalData, copy_from_user_rbp),
        copy_from_user_r12 = const offset_of!(CpuLocalData, copy_from_user_r12),
        copy_from_user_r13 = const offset_of!(CpuLocalData, copy_from_user_r13),
        copy_from_user_r14 = const offset_of!(CpuLocalData, copy_from_user_r14),
        copy_from_user_r15 = const offset_of!(CpuLocalData, copy_from_user_r15),
        copy_from_user_rsp = const offset_of!(CpuLocalData, copy_from_user_rsp),
        access_user_mem_error_pointer = const offset_of!(CpuLocalData, access_user_mem_error_pointer),
        call_closure = sym call_inside_try_access_user_mem,
    )
}

/// This works as a `copy_from_user` and `copy_to_user`, except that you do the copying in your own closure!
/// Remember that if at any point inside the closure a page fault happens, the closure will stop executing and this function will return an error.
/// So don't own any mutexes or anything that implements `Drop` inside the closure.
pub fn try_access_user_mem<T>(f: impl FnOnce() -> Box<T>) -> Result<Box<T>, AccessUserMemError> {
    let f = Box::into_raw(Box::new(Box::new(f) as Box<dyn FnOnce() -> Box<T>>));
    let mut e = MaybeUninit::uninit();
    if has_smap() {
        stac();
    }
    // Safety: the pointer is a boxed closure
    let ret_ptr = unsafe { _try_access_user_mem(f, &mut e) } as *mut T;
    if has_smap() {
        clac();
    }
    if let Some(ret_ptr) = NonNull::new(ret_ptr) {
        Ok({
            // Safety: the return value was a raw box ptr
            unsafe { Box::from_raw(ret_ptr.as_ptr()) }
        })
    } else {
        Err({
            // Safety: The page fault handler initialized it
            unsafe { e.assume_init() }
        })
    }
}
