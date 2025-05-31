use core::{mem::MaybeUninit, slice};

use limine::{memory_map::EntryType, response::MemoryMapResponse};
use talc::{ErrOnOom, Talc, Talck};

use crate::hhdm_offset::HhdmOffset;

// This tells Rust that global allocations will use this static variable's allocation functions
#[global_allocator]
static GLOBAL_ALLOCATOR: 
    // Talck is talc's allocator, but behind a lock, so that it can implement `GlobalAlloc`
    Talck<
        // This tells talc to use a `spin::Mutex` as the locking method
        spin::Mutex<()>, 
        // If talc runs out of memory, it runs an OOM (out of memory) handler. 
        // For now, we do not implement a method of allocating more memory for the global allocator, so we just error on OOM
        ErrOnOom
    > =
    Talck::new(
        // Initially, there is no memory backing `Talc`. We will add memory at run time
        Talc::new(ErrOnOom)
    );

/// Finds unused physical memory for the global allocator and initializes the global allocator
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init(memory_map: &'static MemoryMapResponse, hhdm_offset: HhdmOffset) {
    let global_allocator_size = {
        // 4 MiB
        4 * 0x400 * 0x400
    };
    let global_allocator_physical_start = memory_map
        .entries()
        .iter()
        .find(|entry| {
            entry.entry_type == EntryType::USABLE && entry.length >= global_allocator_size
        })
        .unwrap()
        .base;

    // Safety: We've reserved the physical memory and it is already offset mapped
    let global_allocator_mem = unsafe {
        slice::from_raw_parts_mut::<MaybeUninit<u8>>(
            (u64::from(hhdm_offset) + global_allocator_physical_start) as *mut _,
            global_allocator_size as usize,
        )
    };
    let mut talc = GLOBAL_ALLOCATOR.lock();
    let span = global_allocator_mem.into();
    // Safety: We got the span from valid memory
    unsafe { talc.claim(span) }.unwrap();
}
