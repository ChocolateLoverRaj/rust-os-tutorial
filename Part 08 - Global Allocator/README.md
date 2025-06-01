# Global Allocator
In `x86_64` (and pretty much every architecture), there is physical memory and virtual memory. Physical memory is the actual RAM, and also memory-mapped I/O (one example is the [HPET](https://wiki.osdev.org/HPET), which we will program later). Virtual memory is what our code references. Virtual memory is mapped to physical memory, and the mappings are programmed using page tables. See https://os.phil-opp.com/paging-introduction/ for a more in-depth explanation.

When we write Rust code in `std`, we have data types such as `Box`, `Vec`, `Rc`, and `Arc`. However, in `no_std`, we need an allocator to use those types. In `no_std`, there is no allocator included, and we need to provide our own. See https://os.phil-opp.com/allocator-designs/ for more detailed information about what allocators are.

An allocator basically has a pool of memory (think of it as a `&mut [MaybeUninit<u8>]`) which it allocates towards any data type used by any code. The allocator has to keep track of which parts of memory are allocated, and be able to allocate, deallocate, and (optional, but useful for performance) grow / shrink already allocated memory regions. We don't have to implement an allocator because there are many existing crates that do it for us. We will use the `talc` crate:
```toml
talc = "4.4.2"
```
To enable using `alloc`, we need to add in `main.rs`:
```rs
extern crate alloc;
```
Create a file `memory.rs`:
```rs
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
```

## Finding physical memory
We need to find physical memory to use for our global allocator at run time. For this, we will use Limine's Memory Map feature: 
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
```
Let's create an `init` function in `memory.rs`:
```rs
pub unsafe fn init(memory_map: &'static MemoryMapResponse) {
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
}
```

## Getting the virtual address of the physical memory
Now we found physical memory, but to access it, we need to access it through virtual memory. Limine [offset maps](https://os.phil-opp.com/paging-implementation/#map-at-a-fixed-offset) all `EntryType::USABLE` memory. We just need to know the offset, and then add the offset to the physical memory. To get the offset, we use Limine's HHDM (Higher Half Direct Map) feature:
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
```

## HHDM offset wrapper type
Our init function will need the two Limine requests mentioned earlier. The HHDM response is basically just a `u64`, but let's create a wrapper type so that we don't accidentally treat a diffeent `u64` as the HHDM offset. Create a file `hhdm_offset.rs`:
```rs
/// A wrapper around u64 that represents the actual HHDM offset, and cannot be accidentally made.
/// Remember though that even though this wraps unsafeness in safeness, it is only safe if the assumption that all available memory is mapped in the current Cr3 value according to the HHDM offset (and cache is not invalid)
#[derive(Clone, Copy)]
pub struct HhdmOffset(u64);
```
Now let's implement the `Debug` trait, printing it as hex:
```rs
impl Debug for HhdmOffset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HhdmOffset(0x{:X})", self.0)
    }
}
```
To construct a `HhdmOffset`, we can require having a HHDM response from Limine directly:
```rs
impl From<&'static HhdmResponse> for HhdmOffset {
    fn from(value: &'static HhdmResponse) -> Self {
        Self(value.offset())
    }
}
```

## Initializing the global allocator
Let's add an input, `hhdm_offset: HhdmOffset` to our `init` function. Then, we can create a slice for our memory:
```rs
// Safety: We've reserved the physical memory and it is already offset mapped
let global_allocator_mem = unsafe {
    slice::from_raw_parts_mut::<MaybeUninit<u8>>(
        (u64::from(hhdm_offset) + global_allocator_physical_start) as *mut _,
        global_allocator_size as usize,
    )
};
```
Then, we can give this slice to `talc` to use:
```rs
let mut talc = GLOBAL_ALLOCATOR.lock();
let span = global_allocator_mem.into();
// Safety: We got the span from valid memory
unsafe { talc.claim(span) }.unwrap();
```
Now in `main.rs`, after initializing the logger:
```rs
let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
let hhdm_offset = HHDM_REQUEST.get_response().unwrap().into();
// Safety: we are initializing this for the first time
unsafe { memory::init(memory_map, hhdm_offset) };
```

## Trying it out
After `memory::init`, we can now use data types that need the global allocator. Try adding this:
```rs
let v = (0..5)
    .map(|i| alloc::boxed::Box::new(i))
    .collect::<alloc::vec::Vec<_>>();
let v_ptr_range = v.as_ptr_range();
let contents = v
    .iter()
    .map(|b| {
        let b_ptr = &**b;
        alloc::format!("Box pointer: {b_ptr:p}. Contents: {b}")
    })
    .collect::<alloc::vec::Vec<_>>();
log::info!("Vec: {v_ptr_range:?}. Contents: {contents:#?}");
```
Here we use `Vec`, `Box`, and `String` (which `format!` allocates). The output should look like this:
```
INFO  Vec: 0xffff800000100410..0xffff800000100438. Contents: [
    "Box pointer: 0xffff800000100440. Contents: 0",
    "Box pointer: 0xffff800000100458. Contents: 1",
    "Box pointer: 0xffff800000100470. Contents: 2",
    "Box pointer: 0xffff800000100488. Contents: 3",
    "Box pointer: 0xffff8000001004a0. Contents: 4",
]
```
We can see the pointers the `talc` assigned to our `Vec` and `Box`es.

# Learn More
- https://os.phil-opp.com/paging-introduction/
- https://os.phil-opp.com/paging-implementation
- https://os.phil-opp.com/heap-allocation/
- https://os.phil-opp.com/allocator-designs/
