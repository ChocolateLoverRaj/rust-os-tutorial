# Managing Memory
In `x86_64` (and pretty much every architecture), there is physical memory and virtual memory. Physical memory is the actual RAM, and also memory-mapped I/O (one example is the [HPET](https://wiki.osdev.org/HPET), which we will program later). Virtual memory is what our code references. Virtual memory is mapped to physical memory, and the mappings are programmed using page tables. See https://os.phil-opp.com/paging-introduction/ for a more in-depth explanation.

At this stage in our OS boot process, Limine has set up page tables for us. Limine mapped all usable (physical) memory for us so that we can access it through virtual memory. Limine mapped of the Limine responses, the stacks (one for every CPU), and the executable file itself. But we will be modifying the page tables. 

First, we will allocate some (physical) memory (which we access through virtual memory), to use for a global allocator. When we write Rust code in `std`, we have data types such as `Box`, `Vec`, `Rc`, and `Arc`. However, in `no_std`, we need an allocator to use those types. In `no_std`, there is no allocator included, and we need to provide our own. See https://os.phil-opp.com/allocator-designs/ for more detailed information about what allocators are.

Later, we'll be modifying page tables again to access ACPI tables (don't worry, this will be explained in a future part), as well as access other memory-mapped I/O.

## Memory management-related Limine features
We'll be adding Limine requests to help us manage memory.

### HHDM (Higher Half Direct Map)
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
```
Limine sets up page tables so that we can access physical memory addresses through virtual addresses mapped at the physical address + an offset. See https://os.phil-opp.com/paging-implementation/#map-at-a-fixed-offset. We use the HHDM request to get that offset.

### Memory Map
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
```
The memory map request tells us about physical memory. It tells us which parts of it are free to use by our kernel, which parts are used by Limine (which we currently can't modify or remove because the Limine responses, and our kernel's stacks, are in it), and which parts are used by other stuff (which we can't use).

## Setting up our own page tables
We're going to be mapping virtual memory to physical memory. Currently, we don't really know which parts in virtual memory are already mapped. It could cause issues if we try to map a page which is already mapped. So we'll create a new, blank L4 page table. That way, we know exactly what should and shouldn't be used. However, we need to re-create the mappings that Limine made. Create a file `memory.rs`:
```rs
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init() {}
```
### HHDM offset wrapper type
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
This ensures that we can't accidentally construct it with the wrong `u64`. Let's also implement converting to a `u64`, since the inner `u64` is not `pub`:
```rs
impl From<HhdmOffset> for u64 {
    fn from(value: HhdmOffset) -> Self {
        value.0
    }
}
```
Then we can update our memory init function:
```rs
pub unsafe fn init(memory_map: &'static MemoryMapResponse, hhdm_offset: HhdmOffset) {}
```

### Finding usable physical memory
Our memory init function will do two things: it will set up a global allocator, and create a new page table. First, we need to find usable physical memory for our global allocator. To make things easy for us, we'll look for *contiguous* physical memory so that we can just use the offset mapping to access it through virtual addresses. Note that it's possible that we can't find a contiguous section of usable physical memory because of gaps. To keep things simple, we'll just assume that there will be a contiguous section. We'll start with allocating 4 MiB for the global allocator.
```rs
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
```

### Implementing a frame allocator
Next, we'll need to implement the `FrameAllocator` trait. We need to be careful, because as we allocate usable memory to page tables, that memory isn't usable anymore, it's used. If we had a global allocator, we could've kept a `Vec` of used frames. But our global allocator isn't initialized yet. Instead, we'll do the following:
- Allocate frames, keeping track of the *number* we allocated
- Initialize the global allocator
- Re-iterate through all of the allocated frames, marking them as used

Let's implement an `Iterator` for getting usable frames. Create a file `initial_usable_frames_iterator.rs`. When finding usable frames, we need to keep alignment in mind (4 KiB), and also make sure to not overlap with the physical memory reserved for the global allocator. This can get complicated, so let's use the power of iterators and make a helper function that will get rid of the already-reserved memory for us. Create a file `cut_range.rs`:
```rs
use core::ops::RangeInclusive;

pub trait CutRange<T> {
    fn cut(self, cut_out: &T) -> impl Iterator<Item = T>;
}

impl CutRange<RangeInclusive<u64>> for RangeInclusive<u64> {
    fn cut(self, cut_out: &RangeInclusive<u64>) -> impl Iterator<Item = RangeInclusive<u64>> {
        let result: heapless::Vec<_, 2> = if self.contains(cut_out.start()) {
            if self.end() < cut_out.end() {
                heapless::Vec::from_slice(&[
                    *self.start()..=*cut_out.start() - 1,
                    *cut_out.end() + 1..=*self.end(),
                ])
                .unwrap()
            } else {
                if let Some(end_inclusive) = cut_out.start().checked_sub(1) {
                    heapless::Vec::from_slice(&[*self.start()..=end_inclusive]).unwrap()
                } else {
                    Default::default()
                }
            }
        } else if self.contains(cut_out.end()) {
            heapless::Vec::from_slice(&[*cut_out.end() + 1..=*self.end()]).unwrap()
        } else {
            heapless::Vec::from_slice(&[self]).unwrap()
        };
        result.into_iter()
    }
}
```
What this does is take a `RangeInclusive`, and cut out another `RangeInclusive` from it, which can result in 0, 1, or 2 new `RangeInclusive`s. To return a variable-length iterator, we're using the `heapless` crate. Add it as a dependency:
```toml
heapless = "0.8.0"
```
In `initial_usable_frames_iterator.rs`, create the struct:
```rs
pub struct InitialUsableFramesIterator {
    reserved_range: RangeInclusive<u64>,
    allocated_frames: u64,
    memory_map: &'static MemoryMapResponse,
}
```
Then lets implement a `new` function:
```rs
impl InitialUsableFramesIterator {
    pub fn new(
        memory_map: &'static MemoryMapResponse,
        reserved_range: RangeInclusive<u64>,
    ) -> Self {
        Self {
            reserved_range,
            allocated_frames: 0,
            memory_map,
        }
    }
}
```
And a function to get the number of frames allocated:
```rs
impl InitialUsableFramesIterator {
    pub fn allocated_frames(&self) -> u64 {
        self.allocated_frames
    }
}
```
Now let's implement `Iterator`:
```rs
impl Iterator for InitialUsableFramesIterator {
    type Item = PhysFrame<Size4KiB>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut skipped_frames = 0;
        let frame_start = self
            .memory_map
            .entries()
            .iter()
            .filter_map(|entry| {
                if entry.entry_type == EntryType::USABLE {
                    Some(entry.base..=entry.base + (entry.length - 1))
                } else {
                    None
                }
            })
            .flat_map(|range| range.cut(&self.reserved_range))
            .find_map(|entry| {
                let first_frame_start = entry.start().next_multiple_of(Size4KiB::SIZE);
                let full_frames = (entry.end() - first_frame_start + 1) / Size4KiB::SIZE;
                let frames_left_to_skip = self.allocated_frames - skipped_frames;
                let frames_skipped_in_this_entry = frames_left_to_skip.min(full_frames);
                skipped_frames += frames_skipped_in_this_entry;
                if frames_skipped_in_this_entry < full_frames {
                    Some(first_frame_start + frames_skipped_in_this_entry * Size4KiB::SIZE)
                } else {
                    None
                }
            })?;
        self.allocated_frames += 1;
        Some(PhysFrame::from_start_address(PhysAddr::new(frame_start)).unwrap())
    }
}
```
Every time we get the next frame, we start iterating through the memory map, cutting out the reserved range and skipping the frames that we already allocated.

Now create `initial_frame_allocator.rs`:
```rs
pub struct InitialFrameAllocator {
    iterator: InitialUsableFramesIterator,
    memory_map: &'static MemoryMapResponse,
    reserved_range: RangeInclusive<u64>,
}
```
We'll implement the `unsafe` `FrameAllocator` trait just as a wrapper around the iterator:
```rs
unsafe impl FrameAllocator<Size4KiB> for InitialFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.iterator.next()
    }
}
```
However, this trait is `unsafe` for a reason. Whatever frame it returns ends up actually being used to allocate a frame for a page table, so it must be valid. To enforce this, we'll make the `new` method unsafe:
```rs
impl InitialFrameAllocator {
    /// # Safety
    /// You must not accidentally create two of these, because that will allocate the same frames
    pub unsafe fn new(
        memory_map: &'static MemoryMapResponse,
        reserved_range: RangeInclusive<u64>,
    ) -> Self {
        Self {
            iterator: InitialUsableFramesIterator::new(memory_map, reserved_range.clone()),
            memory_map,
            reserved_range,
        }
    }
}
```
And, since it's also the caller's responsiblity to keep track of the used frames, we'll create a `finish` function which takes `self`, so that more frames cannot be accidentally allocated later:
```rs
impl InitialFrameAllocator {
    /// Finish using this as a frame allocator, and get an iterator of allocated frames so that you can mark them as used
    pub fn finish(self) -> impl Iterator<Item = PhysFrame<Size4KiB>> {
        InitialUsableFramesIterator::new(self.memory_map, self.reserved_range)
            .take(self.iterator.allocated_frames() as usize)
    }
}
```

### Mapping pages
Back in `memory.rs`, let's create the initial allocator:
```rs
// Safety: No frames have been allocated yet
let mut frame_allocator = unsafe {
    InitialFrameAllocator::new(
        memory_map,
        global_allocator_physical_start
            ..=global_allocator_physical_start + (global_allocator_size - 1),
    )
};
```
And allocate a new top level page table:
```rs
let new_l4_frame = frame_allocator.allocate_frame().unwrap();
```
And then create a `&mut PageTable` for it:
```rs
// Safety: The allocated frame is in usable memory, which is offset mapped
let new_l4_page_table = unsafe {
    VirtAddr::new(u64::from(hhdm_offset) + new_l4_frame.start_address().as_u64())
        .as_mut_ptr::<MaybeUninit<PageTable>>()
        .as_mut()
        .unwrap()
        .write(Default::default())
};
```
Notice that we are using `MaybeUninit` to make sure we remember to initialize our page table and `MaybeUninit::write` to create an empty page table. If we don't, there is a chance that the page table could have existing invalid mappings.

We use the `OffsetPageTable` helper to create mappings:
```rs
// Safety: We are only using usable memory, which is offset mapped
let mut new_offset_page_table =
    unsafe { OffsetPageTable::new(new_l4_page_table, VirtAddr::new(hhdm_offset.into())) };
```
We'll start by mapping everything that Limine has already offset mapped. We'll use 1 GiB frames to minimize the number of pages we have to map:
```rs
// Offset map everything that is currently offset mapped
let mut last_mapped_address = None::<PhysAddr>;
for entry in memory_map.entries() {
    if [
        EntryType::USABLE,
        EntryType::BOOTLOADER_RECLAIMABLE,
        EntryType::EXECUTABLE_AND_MODULES,
        EntryType::FRAMEBUFFER,
    ]
    .contains(&entry.entry_type)
    {
        let range_to_map = {
            let first = PhysAddr::new(entry.base);
            let last = first + (entry.length - 1);
            match last_mapped_address {
                Some(last_mapped_address) => {
                    if first > last_mapped_address {
                        Some(first..=last)
                    } else if last > last_mapped_address {
                        Some(last_mapped_address + 1..=last)
                    } else {
                        None
                    }
                }
                None => Some(first..=last),
            }
        };
        if let Some(range_to_map) = range_to_map {
            let first_frame = PhysFrame::<Size1GiB>::containing_address(*range_to_map.start());
            let last_frame = PhysFrame::<Size1GiB>::containing_address(*range_to_map.end());
            let page_count = last_frame - first_frame + 1;

            for i in 0..page_count {
                let frame = first_frame + i;
                let page = Page::<Size1GiB>::from_start_address(VirtAddr::new(
                    frame.start_address().as_u64() + u64::from(hhdm_offset),
                ))
                .unwrap();
                unsafe {
                    new_offset_page_table
                        .map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut frame_allocator,
                        )
                        .unwrap()
                        // Cache will be reloaded anyways when we change Cr3
                        .ignore()
                };
            }
            last_mapped_address = Some(last_frame.start_address() + (Size1GiB::SIZE - 1));
        }
    }
}
```
We don't need to worry about flushing the cache when mapping the pages because the page tables we are modifying are not yet active. The CPU will flush all cache when we write to `Cr3` later.

Next, we can't forget about the kernel itself, which is mapped somewhere in the top 2 GiB. Instead of re-creating these page tables, we will just reuse the L3 page table that Limine made. So we'll be using the top 512 GiB of virtual memory for the kernel, even though at most the kernel will use the top 2 GiB. This is fine because we still have plenty of virtual memory space. Later, if we really wanted to, we could make a fresh L3 table and just copy the last 2 entries (which map 1 GiB each) from Limine's highest L3 table.
```rs
// We must map the kernel, which lies in the top 2 GiB of virtual memory. We can just reuse Limine's mappings for the top 512 GiB
let (current_l4_frame, cr3_flags) = Cr3::read();
let current_l4_page_table = unsafe {
    VirtAddr::new(u64::from(hhdm_offset) + current_l4_frame.start_address().as_u64())
        .as_mut_ptr::<PageTable>()
        .as_mut()
        .unwrap()
};
new_l4_page_table[511].clone_from(&current_l4_page_table[511]);
```
### Switching page tables
Now that we have mapped everything that we will be referencing, we can switch to the new top level page table:
```rs
// Safety: Everything that needs to be mapped is mapped
unsafe { Cr3::write(new_l4_frame, cr3_flags) };
```

## Global Allocator
Now that we've reserved some physical memory, and it's accessible through the offset mapping, let's initialize the global allocator! We'll use the `linked_list_allocator` crate to do the actual allocation logic for us:
```toml
linked_list_allocator = "0.10.5"
```
For now, our kernel will have a fixed amount of memory reserved for the global allocator. If we want to make a proper kernel, we need to have a method of increasing the memory available to use by the global allocator when we run out. But we can do that later.

Let's
```rs
use linked_list_allocator::LockedHeap;
```
And then define our global allocator:
```rs
#[global_allocator]
static GLOBAL_ALLOCATOR: LockedHeap = LockedHeap::empty();
```
`#[global_allocator]` tells Rust to use this global allocator whenever we allocate memory, such as creating a `Box` and adding items to a `Vec`.

In our memory init function, add
```rs
// Safety: We've reserved the physical memory and it is already offset mapped
unsafe {
    GLOBAL_ALLOCATOR.lock().init(
        VirtAddr::new(u64::from(hhdm_offset) + global_allocator_physical_start).as_mut_ptr(),
        global_allocator_size as usize,
    )
};
```

At this point, we can start using `alloc` types:
```rs
let b = alloc::boxed::Box::new(234);
log::info!("Box: {:p} containing {:?}", b, b);
```

## Keeping track of physical memory
First, let's create some enums:
```rs
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
}
```
This way, in addition to finding out which memory sections are usable, we can get more detailed memory usage stats.

Next, let's create a struct that will hold information related to our memory:
```rs
pub struct Memory {
    pub physical_memory: Spinlock<NoditMap<u64, Interval<u64>, MemoryType>>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}
```
We will use the `nodit` crate, which provides `NoditMap`, which let's us map ranges to values.
```toml
nodit = "0.9.2"
```

We need to have a global variable for this, but it won't be initialized at the start. The `conquer-once` crate is great for this:
```toml
conquer-once = { version = "0.4.0", default-features = false }
```
We can
```rs
use conquer_once::noblock::OnceCell;
```
which lets us initialize a global variable later and then concurrently access it once it's initialized.
```rs
pub static MEMORY: OnceCell<Memory> = OnceCell::uninit();
```
In our `init` function, we initialize `MEMORY`:
```rs
// Now let's keep track of the physical memory used
let mut physical_memory = NoditMap::default();

log::debug!("Physical memory usage: {:#X?}", physical_memory);

MEMORY
    .try_init_once(|| Memory {
        physical_memory: Spinlock::new(physical_memory),
        new_kernel_cr3: new_l4_frame,
        new_kernel_cr3_flags: cr3_flags,
    })
    .unwrap();
```
Before putting `physical_memory` in a mutex, let's add the ranges to it:
```rs
// We start with the state when Limine booted our kernel
for entry in memory_map.entries() {
    let should_insert = match entry.entry_type {
        EntryType::USABLE => Some(MemoryType::Usable),
        EntryType::BOOTLOADER_RECLAIMABLE => Some(MemoryType::UsedByLimine),
        _ => {
            // The entry might overlap, so let's not add it
            None
        }
    };
    if let Some(memory_type) = should_insert {
        physical_memory
            // Although they are guaranteed to not overlap and be ascending, Limine doesn't specify that they aren't guaranteed to not be touching even if they are the same.
            .insert_merge_touching_if_values_equal(
                (entry.base..entry.base + entry.length).into(),
                memory_type,
            )
            .unwrap();
    }
}
```
Limine states:
> Usable and bootloader reclaimable entries are guaranteed not to overlap with any other entry. To the contrary, all non-usable entries (including executable/modules) are not guaranteed any alignment, nor is it guaranteed that they do not overlap other entries.

For this reason, we will not add other entries to our map, because that would be inaccurate if the entries overlap, overwriting each other.

Next, we mark the frames allocated by our initial frame allocator as used:
```rs
// We track the used frames for page tables
for frame in frame_allocator.finish() {
    let _ = physical_memory.insert_overwrite(
        {
            let start = frame.start_address().as_u64();
            start..=start + (frame.size() - 1)
        }
        .into(),
        MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
    );
}
```
And finally, mark the global allocator's heap as used:
```rs
// We track the memory used for the global allocator
let _ = physical_memory.insert_overwrite(
    (global_allocator_physical_start
        ..=global_allocator_physical_start + (global_allocator_size - 1))
        .into(),
    MemoryType::UsedByKernel(KernelMemoryUsageType::GlobalAllocatorHeap),
);
```

## Back in `main.rs`
After initializing the logger, before processing the MP request, let's initialize the memory:
```rs
let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
let hhdm_offset = HHDM_REQUEST.get_response().unwrap().into();
// Safety: we are initializing this for the first time
unsafe { memory::init(memory_map, hhdm_offset) };
```
And at the top of `entry_point_from_limine_mp`, add:
```rs
let memory = MEMORY.try_get().unwrap();
// Safety: This function is only executed after memory is initialized
unsafe { Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags) };
```
Every CPU has its own `Cr3` register. We changed the `Cr3` value for the BSP, but we also have to change it for the APs. 

Now we should see an output like this:
```
INFO  Hello World!
DEBUG Physical memory usage: NoditMap {
    inner: {
        Interval {
            start: 0x0,
            end: 0xFFF,
        }: UsedByKernel(
            PageTables,
        ),
        Interval {
            start: 0x1000,
            end: 0x1FFF,
        }: UsedByLimine,
        Interval {
            start: 0x2000,
            end: 0x2FFF,
        }: UsedByKernel(
            PageTables,
        ),
        Interval {
            start: 0x3000,
            end: 0x9FFFF,
        }: Usable,
        Interval {
            start: 0x100000,
            end: 0x4FFFFF,
        }: UsedByKernel(
            GlobalAllocatorHeap,
        ),
        Interval {
            start: 0x500000,
            end: 0x7FFFFF,
        }: Usable,
        Interval {
            start: 0x808000,
            end: 0x80AFFF,
        }: Usable,
        Interval {
            start: 0x80C000,
            end: 0x810FFF,
        }: Usable,
        Interval {
            start: 0x900000,
            end: 0x23BEFFF,
        }: Usable,
        Interval {
            start: 0x23F5000,
            end: 0x2623FFF,
        }: UsedByLimine,
        Interval {
            start: 0x2624000,
            end: 0x263EFFF,
        }: Usable,
        Interval {
            start: 0x263F000,
            end: 0x265BFFF,
        }: UsedByLimine,
        Interval {
            start: 0x265C000,
            end: 0x3B7BFFF,
        }: Usable,
        Interval {
            start: 0x3B7C000,
            end: 0x3BE6FFF,
        }: UsedByLimine,
        Interval {
            start: 0x3BE7000,
            end: 0x66CFFFF,
        }: Usable,
        Interval {
            start: 0x66D5000,
            end: 0x66E6FFF,
        }: Usable,
        Interval {
            start: 0x66E7000,
            end: 0x6757FFF,
        }: UsedByLimine,
        Interval {
            start: 0x6760000,
            end: 0x6845FFF,
        }: Usable,
        Interval {
            start: 0x6846000,
            end: 0x6848FFF,
        }: UsedByLimine,
        Interval {
            start: 0x684C000,
            end: 0x74ECFFF,
        }: Usable,
        Interval {
            start: 0x77FF000,
            end: 0x7DFFFFF,
        }: Usable,
        Interval {
            start: 0x7E00000,
            end: 0x7E6DFFF,
        }: UsedByLimine,
        Interval {
            start: 0x7E6E000,
            end: 0x7EB0FFF,
        }: Usable,
        Interval {
            start: 0x7EB7000,
            end: 0x7EEBFFF,
        }: Usable,
    },
    phantom: PhantomData<u64>,
}
INFO  CPU Count: 2
INFO  Hello from CPU 1
```
Now let's remove the `log::debug!("Physical memory usage: {:#X?}", physical_memory);` because it takes up so much space in the output.

# Learn More
- https://os.phil-opp.com/paging-introduction/
- https://os.phil-opp.com/paging-implementation
- https://os.phil-opp.com/heap-allocation/
- https://os.phil-opp.com/allocator-designs/
