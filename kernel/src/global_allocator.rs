use core::{cell::SyncUnsafeCell, mem::MaybeUninit};

use linked_list_allocator::LockedHeap;

const GLOBAL_ALLOCATOR_MEMORY_SIZE: usize = {
    // 1 MiB
    1 * 0x400 * 0x400
};

static GLOBAL_ALLOCATOR_MEMORY: SyncUnsafeCell<[MaybeUninit<u8>; GLOBAL_ALLOCATOR_MEMORY_SIZE]> =
    SyncUnsafeCell::new([MaybeUninit::uninit(); GLOBAL_ALLOCATOR_MEMORY_SIZE]);

#[global_allocator]
static GLOBAL_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// # Safety
/// This function must be called exactly once
pub unsafe fn init() {
    GLOBAL_ALLOCATOR
        .lock()
        .init_from_slice(unsafe { GLOBAL_ALLOCATOR_MEMORY.get().as_mut().unwrap() });
}
