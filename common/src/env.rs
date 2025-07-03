use core::ptr::NonNull;

#[derive(Debug, Clone, Copy)]
pub struct EnvEntry {
    /// Generate a random u64 to avoid conflicts
    pub key: u64,
    /// Use this as data or as a pointer, it's up to you
    pub value: u64,
}

/// # Safety
/// - Your program must be executed with the stack pointer pointing to a u64 for the number of entries followed by the entries themselves.
/// - You must pass the initial stack pointer. This requires using [`core::arch::naked_asm`] as your entry function.
pub unsafe fn env_entries(initial_rsp: *mut ()) -> NonNull<[EnvEntry]> {
    let entry_count_ptr = initial_rsp.cast::<u64>();
    // Safety: The u64 above the rsp is the number of entries
    let entry_count = unsafe { entry_count_ptr.read() } as usize;
    let entries_ptr = (entry_count_ptr as usize + size_of::<u64>()) as *mut EnvEntry;
    // Safety: Above the count is the entries themselves
    let entries_slice = unsafe { core::slice::from_raw_parts_mut(entries_ptr, entry_count) };
    NonNull::new(entries_slice).unwrap()
}
