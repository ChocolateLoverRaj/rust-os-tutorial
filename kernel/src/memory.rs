use core::{
    mem::MaybeUninit,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use limine::{memory_map::EntryType, response::MemoryMapResponse};
use talc::{ErrOnOom, Talc, Talck};
use x86_64::PhysAddr;

use crate::OffsetMappedPhysAddr;

// This tells Rust that global allocations will use this static variable's allocation functions
// Talck is talc's allocator, but behind a lock, so that it can implement `GlobalAlloc`
// We tell talc to use a `spin::Mutex` as the locking method
// If talc runs out of memory, it runs an OOM (out of memory) handler.
// For now, we do not implement a method of allocating more memory for the global allocator, so we just error on OOM
#[global_allocator]
static GLOBAL_ALLOCATOR: Talck<spin::Mutex<()>, ErrOnOom> = Talck::new({
    // Initially, there is no memory backing `Talc`. We will add memory at run time
    Talc::new(ErrOnOom)
});

/// Finds unused physical memory for the global allocator and initializes the global allocator
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init(memory_map: &'static MemoryMapResponse) {
    let global_allocator_size = {
        // 4 MiB
        4 * 0x400 * 0x400
    };
    let global_allocator_physical_start = PhysAddr::new(
        memory_map
            .entries()
            .iter()
            .find(|entry| {
                entry.entry_type == EntryType::USABLE && entry.length >= global_allocator_size
            })
            .unwrap()
            .base,
    );

    let global_allocator_mem = {
        let mut ptr = NonNull::new(slice_from_raw_parts_mut(
            global_allocator_physical_start
                .offset_mapped()
                .as_mut_ptr::<MaybeUninit<u8>>(),
            global_allocator_size as usize,
        ))
        .unwrap();
        // Safety: We've reserved the physical memory and it is already offset mapped
        unsafe { ptr.as_mut() }
    };
    let mut talc = GLOBAL_ALLOCATOR.lock();
    let span = global_allocator_mem.into();
    // Safety: We got the span from valid memory
    unsafe { talc.claim(span) }.unwrap();
}
