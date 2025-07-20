use core::{ops::Deref, ptr::NonNull};

use alloc::collections::btree_map::BTreeMap;
use common::EnvEntry;

/// # Safety
/// - Your program must be executed with the stack pointer pointing to a u64 for the number of entries followed by the entries themselves.
/// - You must pass the initial stack pointer. This requires using [`core::arch::naked_asm`] as your entry function.
unsafe fn env_entries(initial_rsp: NonNull<()>) -> NonNull<[EnvEntry]> {
    let entry_count_ptr = initial_rsp.cast::<u64>();
    // Safety: The u64 above the rsp is the number of entries
    let entry_count = unsafe { entry_count_ptr.read() } as usize;
    let entries_ptr = (entry_count_ptr.as_ptr().addr() + size_of::<u64>()) as *mut EnvEntry;
    // Safety: Above the count is the entries themselves
    let entries_slice = unsafe { core::slice::from_raw_parts_mut(entries_ptr, entry_count) };
    NonNull::new(entries_slice).unwrap()
}

/// A wrapper around a map of env entries
#[derive(Debug)]
pub struct EnvEntries {
    entries: BTreeMap<u64, u64>,
}

impl EnvEntries {
    /// # Safety
    /// - Your program must be executed with the stack pointer pointing to a u64 for the number of entries followed by the entries themselves.
    /// - You must pass the initial stack pointer. This requires using [`core::arch::naked_asm`] as your entry function.
    pub unsafe fn from_initial_rsp(initial_rsp: NonNull<()>) -> Self {
        let env_entries_ptr = unsafe { env_entries(initial_rsp) };
        let env_entries = unsafe { env_entries_ptr.as_ref() };
        let entries = env_entries
            .iter()
            .map(|entry| (entry.key, entry.value))
            .collect();
        // Technically since we copied the entries into a `BTreeMap` we could reuse the memory for the env entries, but we currently don't do that.
        Self { entries }
    }
}

impl Deref for EnvEntries {
    type Target = BTreeMap<u64, u64>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
