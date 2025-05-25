# Global Allocator
When we write Rust code in `std`, we have data types such as `Box`, `Vec`, `Rc`, and `Arc`. However, in `no_std`, we need an allocator to use those types. In `no_std`, there is no allocator included, and we need to provide our own. See https://os.phil-opp.com/allocator-designs/ for more detailed information about what allocators are. Let's add a global allocator to our kernel so that we can use those data types that require an allocator (we will need them for future parts). 

We'll use the `linked_list_allocator` crate to do the actual allocation logic for us:
```toml
linked_list_allocator = "0.10.5"
```
Create a file `global_allocator.rs`:
```rs
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
```
And in `main.rs` add:
```rs
#![feature(sync_unsafe_cell)]
```
We use `SyncUnsafeCell` as a data type with interior mutability, except that any access to internal data requires `unsafe`, since there are no protections or guarantees from unsafe memory access.

For now, our kernel will have a fixed amount of memory reserved for the global allocator. If we want to make a proper kernel, we need to have a method of increasing the memory available to use by the global allocator when we run out. But for now, we will just start simple.

And before we can use types from `alloc::`, we need to enable it by adding in `main.rs`:
```rs
extern crate alloc;
```
And let's initialize the allocator in our entry function:
```rs
// Safety: we are initializing this for the first time
unsafe { global_allocator::init() };
```
and we can test it out (after initializing the global allocator):
```rs
let b = alloc::boxed::Box::new(3);
log::info!("Box: {:?} {:p}", b, b);
```
It should output something like
```rs
INFO  Box: 3 0xffffffff8001a320
```
