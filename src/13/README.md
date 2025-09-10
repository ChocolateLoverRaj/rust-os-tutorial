# Managing Physical and Virtual Memory
I will assume that you already know about physical and virtual memory, and how paging works on x86_64 to map virtual memory to physical memory. You can read the following to learn these things:
- <https://os.phil-opp.com/paging-introduction/>
- <https://os.phil-opp.com/paging-implementation/>
- <https://wiki.osdev.org/Paging>

So, what is the state of virtual and physical memory so far? Limine has given us a list of physical memory regions, which lets us know which ones are available for us to use. We've already used a region of physical memory for our global allocator heap. Limine set up page tables, and we have not modified them yet.

## Re-organizing `memory.rs`
First, let's change `memory.rs` to be `memory/mod.rs` (creating a folder called `memory`). Then create a file `memory/global_allocator.rs`. Basically, move everything that used to be in `memory.rs` to `memory/global_allocator.rs`. And change the `init` function to return a `PhysAddr`. In the end of the function, return `global_allocator_physical_start`. In `memory/mod.rs`, create this fn:
```rs
/// Initializes global allocator, creates new page tables, and switches to new page tables.
/// This function must be called before mapping pages or running our kernel's code on APs.
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init_bsp(memory_map: &'static MemoryMapResponse) {
    let global_allocator_start = unsafe { global_allocator::init(memory_map) };
}
```

## Managing physical memory
Create a file `memory/physical_memory.rs`. Then, add the `nodit` crate:
```toml
nodit = { version = "0.9.2", default-features = false }
```
We will create the following structs:
```rs
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KernelMemoryUsageType {
    PageTables,
    GlobalAllocatorHeap,
}

/// Note that there are other memory types (such as ACPI memory) that are not included here
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MemoryType {
    Usable,
    UsedByLimine,
    UsedByKernel(KernelMemoryUsageType),
}

#[derive(Debug)]
pub struct PhysicalMemory {
    map: NoditMap<u64, Interval<u64>, MemoryType>,
}
```
This way, we can not only keep track of which memory we have used, but also what we used it for (roughly).

Next, let's create a function to create `PhysicalMemory`:
```rs
impl PhysicalMemory {
    pub(super) fn new(
        memory_map: &'static MemoryMapResponse,
        global_allocator_start: PhysAddr,
    ) -> Self {
        Self {
            map: {
                let mut map = NoditMap::default();
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
                        map
                            // Although they are guaranteed to not overlap and be ascending, Limine doesn't specify that they aren't guaranteed to not be touching even if they are the same.
                            .insert_merge_touching_if_values_equal(
                                (entry.base..entry.base + entry.length).into(),
                                memory_type,
                            )
                            .unwrap();
                    }
                }
                // We track the memory used for the global allocator
                let interval = Interval::from(
                    global_allocator_start.as_u64()
                        ..global_allocator_start.as_u64() + global_allocator::GLOBAL_ALLOCATOR_SIZE,
                );
                let _ = map.cut(interval);
                map.insert_merge_touching_if_values_equal(
                    interval,
                    MemoryType::UsedByKernel(KernelMemoryUsageType::GlobalAllocatorHeap),
                )
                .unwrap();
                map
            },
        }
    }
}
```
Don't forget, we have already allocated some physical memory towards the global allocator's heap. We mark that memory as used.

Back in `memory::init_bsp`, let's call the fn:
```rs
let mut physical_memory = PhysicalMemory::new(memory_map, global_allocator_start);
```


## Setting up our own page tables
We're going to be mapping virtual memory to physical memory. Currently, we don't really know which parts in virtual memory are already mapped. It could cause issues if we try to map a page which is already mapped. So we'll create a new, blank L4 page table. That way, we know exactly what should and shouldn't be used. However, we need to re-create the mappings that Limine made.

To manage page tables, we will use the `ez_paging` crate:
```rs
ez_paging = { git = "https://github.com/ChocolateLoverRaj/ez_paging", version = "0.1.0", default-features = false }
```

Before we create page tables, we'll need a way of allocating physical memory towards page tables:
```rs
impl PhysicalMemory {
    pub fn allocate_frame_with_type(
        &mut self,
        page_size: PageSize,
        memory_type: MemoryType,
    ) -> Option<Frame> {
        let aligned_start = self.map.iter().find_map(|(interval, memory_type)| {
            if let MemoryType::Usable = memory_type {
                let aligned_start = interval.start().next_multiple_of(page_size.byte_len_u64());
                let required_end = aligned_start + page_size.byte_len_u64();
                if required_end <= interval.end() {
                    Some(aligned_start)
                } else {
                    None
                }
            } else {
                None
            }
        })?;
        let range = aligned_start..aligned_start + page_size.byte_len_u64();
        let _ = self.map.cut(Interval::from(range.clone()));
        self.map
            .insert_merge_touching_if_values_equal(range.into(), memory_type)
            .unwrap();
        Some(Frame::new(PhysAddr::new(aligned_start), page_size).unwrap())
    }
}
```
We'll also need to implement `x86_64::structures::paging::FrameAllocator`, and have a way of getting a `ez_paging::Owned4KibFrame`:
```rs
pub struct PhysicalMemoryFrameAllocator<'a> {
    physical_memory: &'a mut PhysicalMemory,
    memory_type: MemoryType,
}

impl PhysicalMemoryFrameAllocator<'_> {
    pub fn allocate_4kib_frame(&mut self) -> Option<Owned4KibFrame> {
        let frame = self
            .physical_memory
            .allocate_frame_with_type(PageSize::_4KiB, self.memory_type)?;
        let frame = PhysFrame::from_start_address(frame.start_addr()).unwrap();
        // Safety: we exclusively access the frame
        let frame = unsafe { Owned4KibFrame::new(frame) };
        Some(frame)
    }
}

unsafe impl FrameAllocator<Size4KiB> for PhysicalMemoryFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        Some(self.allocate_4kib_frame()?.into())
    }
}
```
And let's add a method to get a `PhysicalMemoryFrameAllocator`:
```rs
impl PhysicalMemory {
    pub fn get_kernel_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        }
    }
}
```
We're ready to create page tables! 

Create a file `memory/create_page_tables.rs`:
```rs
/// Creates new page tables, but does not switch to them
pub fn create_page_tables(
    memory_map: &'static MemoryMapResponse,
    physical_memory: &mut PhysicalMemory,
) -> (PhysFrame, Cr3Flags, VirtualMemory) {
    todo!()
}
```
For now, ignore that `VirtualMemory` is not defined. Inside the function, we can start off by creating a new top level page table for the kernel:
```rs
let hhdm_offset = hhdm_offset();
let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
let mut l4 = PagingConfig::new(
    // Safety: we don't touch the PAT
    unsafe { ManagedPat::new() },
    hhdm_offset.into(),
)
.new_kernel(frame_allocator.allocate_4kib_frame().unwrap());
```
Next, we need to re-create some mappings that Limine created for its page tables.
```rs
// Offset map everything that is currently offset mapped
let page_size = max_page_size();
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
            let start = PhysAddr::new(entry.base);
            let end = start + entry.length;
            match last_mapped_address {
                Some(last_mapped_address) => {
                    if start > last_mapped_address {
                        Some(start..end)
                    } else if end > last_mapped_address {
                        Some(last_mapped_address + 1..end)
                    } else {
                        None
                    }
                }
                None => Some(start..end),
            }
        };
        if let Some(range_to_map) = range_to_map {
            let first_frame = Frame::new(
                range_to_map.start.align_down(page_size.byte_len_u64()),
                page_size,
            )
            .unwrap();
            let pages_len = range_to_map.end.as_u64().div_ceil(page_size.byte_len_u64())
                - range_to_map.start.as_u64() / page_size.byte_len_u64();

            for i in 0..pages_len {
                let frame = first_frame.offset(i).unwrap();
                let page = frame.offset_mapped();
                let flags = ConfigurableFlags {
                    writable: true,
                    executable: false,
                    pat_memory_type: PatMemoryType::WriteBack,
                };
                unsafe { l4.map_page(page, frame, flags, &mut frame_allocator) }.unwrap();
            }
            last_mapped_address = Some(range_to_map.end.align_up(page_size.byte_len_u64()) - 1);
        }
    }
}
```
We also need to re-map the kernel executable itself somehow. The easiest way is to just reusing Limine's kernel mappings.
```rs
// We must map the kernel, which lies in the top 2 GiB of virtual memory
// We can just reuse Limine's mappings for the top 512 GiB
let (current_l4_frame, cr3_flags) = Cr3::read();
```
The `Cr3` register contains the physical address of the top level page table, as well as some flags. We can use the offset mapping to access the existing level 4 page table:
```rs
let current_l4_page_table = {
    let ptr = NonNull::new(
        current_l4_frame
            .start_address()
            .offset_mapped()
            .as_mut_ptr::<PageTable>(),
    )
    .unwrap();
    // Safety: we are just going to reference it immutably, and nothing is referencing it mutably
    unsafe { ptr.as_ref() }
};
```
Next, we can get a reference to the new level 4 page table
```rs
let new_l4_page_table = {
    let mut ptr = l4.page_table();
    // Safety: we are just going to copy the last entry, and not modify that region's mappings
    unsafe { ptr.as_mut() }
};
```
Finally, we copy the last entry from the current to the new page table:
```rs
new_l4_page_table[511].clone_from(&current_l4_page_table[511]);
```
Then, we will return some information:
```rs
(
    *l4.frame().deref(),
    cr3_flags,
    todo!("Virtual memory")
)
```
Let's call this fn back in `memory::init_bsp`:
```rs
let (new_kernel_cr3, new_kernel_cr3_flags, virtual_memory) =
    create_page_tables(memory_map, &mut physical_memory);
```
And then switch to the new page tables by writing to `Cr3`: 
```rs
// Safety: page tables are ready to be used
unsafe { Cr3::write(new_kernel_cr3, new_kernel_cr3_flags) };
```

## Managing virtual memory
Create `memory/virtual_memory.rs`:
```rs
use ez_paging::ManagedL4PageTable;
use nodit::{Interval, NoditSet};

#[derive(Debug)]
pub struct VirtualMemory {
    #[allow(unused)]
    pub(super) set: NoditSet<u64, Interval<u64>>,
    #[allow(unused)]
    pub(super) l4: ManagedL4PageTable,
}
```
For now, `VirtualMemory` doesn't do much. It just uses a `NoditSet` to keep track of which virtual memory was used, and stores the `ManagedL4PageTable` from the `ez_paging` crate.

Back in `create_page_tables`, we can initialize the `VirtualMemory` where we have our `todo!()`:
```rs
VirtualMemory {
    set: {
        // Now let's keep track of the used virtual memory
        let mut set = NoditSet::default();
        // Let's add all of the offset mapped regions, keeping in mind we used 1 GiB pages
        for entry in memory_map.entries() {
            if [
                EntryType::USABLE,
                EntryType::BOOTLOADER_RECLAIMABLE,
                EntryType::EXECUTABLE_AND_MODULES,
                EntryType::FRAMEBUFFER,
            ]
            .contains(&entry.entry_type)
            {
                let start = u64::from(hhdm_offset)
                    + entry.base / page_size.byte_len_u64() * page_size.byte_len_u64();
                let end = u64::from(hhdm_offset)
                    + (entry.base + (entry.length - 1)) / page_size.byte_len_u64()
                        * page_size.byte_len_u64()
                    + (page_size.byte_len_u64() - 1);
                set.insert_merge_touching_or_overlapping((start..=end).into());
            }
        }
        // Let's add the top 512 GiB
        set.insert_merge_touching(iu(0xFFFFFF8000000000)).unwrap();
        set
    },
    l4,
}
```
We mark all of the memory used for offset mapping as used. We also mark the top 512 GiB as used, since we are reusing the last entry from Limine's level 4 page table.

## Putting it together
Back in `memory/mod.rs`, let's store all memory-related data:
```rs
#[non_exhaustive]
#[derive(Debug)]
pub struct Memory {
    #[allow(unused)]
    pub physical_memory: spin::Mutex<PhysicalMemory>,
    #[allow(unused)]
    pub virtual_memory: spin::Mutex<VirtualMemory>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}

pub static MEMORY: Once<Memory> = Once::new();
```
and in `memory::init_bsp`, initialize it:
```rs
MEMORY.call_once(|| Memory {
    physical_memory: spin::Mutex::new(physical_memory),
    virtual_memory: spin::Mutex::new(virtual_memory),
    new_kernel_cr3,
    new_kernel_cr3_flags,
});
```
Now, in `main.rs`, remove the old `memory::init`, and add:
```rs
// Safety: no page tables were modified before this
unsafe { memory::init_bsp(memory_map) };
```
Now, the BSP will switch to the new page tables. But what about the APs? For them, we can create a simple function in `memory/mod.rs`:
```rs
/// # Safety
/// Must be called on all APs before modifying page tables
pub unsafe fn init_ap() {
    let memory = MEMORY.get().unwrap();
    // Safety: page tables are ready to be used
    unsafe { Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags) };
}
```
The page tables are shared between all CPUs. In the APs, we just need to switch to the new page tables by writing to `Cr3`. In the top of `entry_point_ap`, call it:
```rs
// Safety: we are calling this right away
unsafe { memory::init_ap() };
```

Now, when we run the code the log messages should be logged like before. There should not be any exceptions or crashes.

## Recap
We did the following in this part:
- Kept track of which physical memory is used, and which physical memory is available to allocate
- Kept track of which virtual memory is used
- Created new page tables, with mappings for the offset mapping and the kernel executable
- Switched to the new page tables on all CPUs
