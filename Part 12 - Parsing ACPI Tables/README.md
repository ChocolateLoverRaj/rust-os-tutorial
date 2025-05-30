# Parsing ACPI Tables
ACPI tables is binary data that provides information about the computer to the operating system. We'll need to parse ACPI tables to send interrupts between CPUs, access timers, and more. We'll use the `acpi` crate, which parses the binary data into nice Rust types.
```toml
acpi = "5.2.0"
```
Create a file `acpi.rs`. We'll be using the [`AcpiTables::from_rsdp`](https://docs.rs/acpi/5.2.0/acpi/struct.AcpiTables.html#method.from_rsdp) method. It needs a handler, which maps the ACPI memory, and the address of the [RSDP](https://wiki.osdev.org/RSDP). 

# RSDP request
We can ask Limine for the RSDP address by adding the request:
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
```

## Implementing `AcpiHandler`
For the handler, we'll need to make our own. In `acpi.rs`, add:
```rs
#[derive(Debug, Clone)]
struct KernelAcpiHandler {}

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        todo!()
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        todo!()
    }
}
```
As you can see, we need to implement a function that maps a physical memory region to a virtual memory region. In our handler, we need to:
- Find an unused virtual memory range
- Map the physical memory to the virtual memory range
- Return the mapping information

To modify the page tables, we will need the HHDM offset:
```rs
#[derive(Debug, Clone)]
struct KernelAcpiHandler {
    hhdm_offset: HhdmOffset,
}
```
Next, we'll need to find unused virtual memory.

## Keeping track of used virtual memory
Currently, we don't keep track of virutla memory like we keep track of physical memory. Let's do it now. In the `Memory` struct, add
```rs
pub used_virtual_memory: Spinlock<NoditSet<u64, Interval<u64>>>,
```
And then in the memory init function, add
```rs
// Now let's keep track of the used virtual memory
let mut used_virtual_memory = NoditSet::default();
```
and add
```rs
used_virtual_memory: Spinlock::new(used_virtual_memory)
```
when creating `Memory`. Let's mark all of the offset mapped region as used. Because we will either be using 1 GiB or 2 MiB pages, let's create an internal generic init function:
```rs
fn init_with_page_size<S: PageSize + Debug>(
    memory_map: &'static MemoryMapResponse,
    hhdm_offset: HhdmOffset,
) where
    for<'a> OffsetPageTable<'a>: Mapper<S>,
{
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

    // Safety: No frames have been allocated yet
    let mut frame_allocator = unsafe {
        InitialFrameAllocator::new(
            memory_map,
            global_allocator_physical_start
                ..=global_allocator_physical_start + (global_allocator_size - 1),
        )
    };

    let new_l4_frame = frame_allocator.allocate_frame().unwrap();
    // Safety: The allocated frame is in usable memory, which is offset mapped
    let new_l4_page_table = unsafe {
        VirtAddr::new(u64::from(hhdm_offset) + new_l4_frame.start_address().as_u64())
            .as_mut_ptr::<MaybeUninit<PageTable>>()
            .as_mut()
            .unwrap()
            .write(Default::default())
    };
    // Safety: We are only using usable memory, which is offset mapped
    let mut new_offset_page_table =
        unsafe { OffsetPageTable::new(new_l4_page_table, VirtAddr::new(hhdm_offset.into())) };

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
                let first_frame = PhysFrame::<S>::containing_address(*range_to_map.start());
                let last_frame = PhysFrame::<S>::containing_address(*range_to_map.end());
                let page_count = last_frame - first_frame + 1;

                for i in 0..page_count {
                    let frame = first_frame + i;
                    let page = Page::<S>::from_start_address(VirtAddr::new(
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
                last_mapped_address = Some(last_frame.start_address() + (S::SIZE - 1));
            }
        }
    }

    // We must map the kernel, which lies in the top 2 GiB of virtual memory. We can just reuse Limine's mappings for the top 512 GiB
    let (current_l4_frame, cr3_flags) = Cr3::read();
    let current_l4_page_table = unsafe {
        VirtAddr::new(u64::from(hhdm_offset) + current_l4_frame.start_address().as_u64())
            .as_mut_ptr::<PageTable>()
            .as_mut()
            .unwrap()
    };
    new_l4_page_table[511].clone_from(&current_l4_page_table[511]);

    // Safety: Everything that needs to be mapped is mapped
    unsafe { Cr3::write(new_l4_frame, cr3_flags) };

    // Safety: We've reserved the physical memory and it is already offset mapped
    let global_allocator_mem = unsafe {
        slice::from_raw_parts_mut(
            (u64::from(hhdm_offset) + global_allocator_physical_start) as *mut _,
            global_allocator_size as usize,
        )
    };
    GLOBAL_ALLOCATOR
        .lock()
        .init_from_slice(global_allocator_mem);

    // Now let's keep track of the physical memory used
    let mut physical_memory = NoditMap::default();
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
    // We track the memory used for the global allocator
    let _ = physical_memory.insert_overwrite(
        (global_allocator_physical_start
            ..=global_allocator_physical_start + (global_allocator_size - 1))
            .into(),
        MemoryType::UsedByKernel(KernelMemoryUsageType::GlobalAllocatorHeap),
    );

    // Now let's keep track of the used virtual memory
    let mut used_virtual_memory = NoditSet::default();
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
            let start = u64::from(hhdm_offset) + entry.base / S::SIZE * S::SIZE;
            let end = u64::from(hhdm_offset)
                + (entry.base + (entry.length - 1)) / S::SIZE * S::SIZE
                + (S::SIZE - 1);
            used_virtual_memory.insert_merge_touching_or_overlapping((start..=end).into());
        }
    }
    // Let's add the top 512 GiB
    used_virtual_memory
        .insert_merge_touching(iu(0xFFFFFF8000000000))
        .unwrap();

    MEMORY
        .try_init_once(|| Memory {
            physical_memory: Spinlock::new(PhysicalMemory {
                map: physical_memory,
            }),
            used_virtual_memory: Spinlock::new(used_virtual_memory),
            new_kernel_cr3: new_l4_frame,
            new_kernel_cr3_flags: cr3_flags,
        })
        .unwrap();
}
```
We are also reserving the top 512 GiB since we are reusing Limine's L3 page table

And then let's create a `pub` init function that uses either 1 GiB or 2 MiB pages:
```rs
/// Sets up a new L4 page table, initializes the global allocator, switches Cr3 to the new page table, and initializes `MEMORY`
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init(memory_map: &'static MemoryMapResponse, hhdm_offset: HhdmOffset) {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .unwrap()
        .has_1gib_pages()
    {
        init_with_page_size::<Size1GiB>(memory_map, hhdm_offset);
    } else {
        init_with_page_size::<Size2MiB>(memory_map, hhdm_offset);
    }
}
``` 
All other virtual memory is available and unmapped in our new page tables.

## ACPI handler generic methods
Our ACPI handler will also be mapping and unmapping pages with the page size determined at run time. Let's create generic internal methods:
```rs
impl KernelAcpiHandler {
    fn map_physical_region_with_page_size<S: PageSize + Debug, T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T>
    where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        todo!()
    }

    fn unmap_physical_region_with_page_size<S: PageSize + Debug, T>(
        region: &acpi::PhysicalMapping<Self, T>,
    ) where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        todo!()
    }
}
```
And then the `AcpiHandler` implementation:
```rs
impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        if CpuId::new()
            .get_extended_processor_and_feature_identifiers()
            .unwrap()
            .has_1gib_pages()
        {
            self.map_physical_region_with_page_size::<Size1GiB, T>(physical_address, size)
        } else {
            self.map_physical_region_with_page_size::<Size2MiB, T>(physical_address, size)
        }
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        if CpuId::new()
            .get_extended_processor_and_feature_identifiers()
            .unwrap()
            .has_1gib_pages()
        {
            Self::unmap_physical_region_with_page_size::<Size1GiB, T>(region)
        } else {
            Self::unmap_physical_region_with_page_size::<Size2MiB, T>(region)
        }
    }
}
```

## Finding unused virtual memory
In our `map_physical_region_with_page_size` method, let's lock the physical and virtual memory, and find a contiguous region of virtual memory in the higher half, made up of 1 GiB pages: 
```rs
let memory = MEMORY.try_get().unwrap();
let mut physical_memory = memory.physical_memory.lock();
let mut virtual_higher_half = memory.used_virtual_memory.lock();

let n_pages = (((size + physical_address) as u64).div_ceil(S::SIZE)
    - physical_address as u64 / S::SIZE) as u64;
let start_frame =
    PhysFrame::<S>::containing_address(PhysAddr::new(physical_address as u64));
let start_page = Page::<S>::from_start_address(VirtAddr::new({
    let range = virtual_higher_half
        .gaps_trimmed(iu(0xffff800000000000))
        .find_map(|gap| {
            let aligned_start = gap.start().next_multiple_of(S::SIZE);
            let required_end_inclusive = aligned_start + (n_pages * S::SIZE - 1);
            if required_end_inclusive <= gap.end() {
                Some(aligned_start..=required_end_inclusive)
            } else {
                None
            }
        })
        .unwrap();
    let start = *range.start();
    virtual_higher_half
        .insert_merge_touching(Interval::from(range))
        .unwrap();
    start
}))
.unwrap();
```
We use `insert_merge_touching`, which will panic if our interval overlaps, making it easier to detect virtual memory management bugs.

Next, let's get an `OffsetPageTable`:
```rs
let level_4_table_physical_frame = Cr3::read().0;
let level_4_page_table = unsafe {
    VirtAddr::new(
        u64::from(self.hhdm_offset) + level_4_table_physical_frame.start_address().as_u64(),
    )
    .as_mut_ptr::<PageTable>()
    .as_mut()
    .unwrap()
};
let mut offset_page_table = unsafe {
    OffsetPageTable::new(level_4_page_table, VirtAddr::new(self.hhdm_offset.into()))
};
```
Next, we will use `map_to` to map the pages. But, we'll need a `FrameAllocator`. Last time, we used `InitialFrameAllocator`, but we can't use that anymore. This time, we'll need to implement `FrameAllocator` which allocates based on the physical memory map. 

## New frame allocator
For convenience, let's create a wrapper around the physical memory map which can implement `FrameAllocator`. Create `physical_memory.rs`:
```rs
pub struct PhysicalMemory {
    pub map: NoditMap<u64, Interval<u64>, MemoryType>,
}
```
And let's implement `FrameAllocator`:
```rs
unsafe impl<S: PageSize> FrameAllocator<S> for PhysicalMemory {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let aligned_start = self.map.iter().find_map(|(interval, memory_type)| {
            if let MemoryType::Usable = memory_type {
                let aligned_start = interval.start().next_multiple_of(S::SIZE);
                let required_end_inclusive = aligned_start + (S::SIZE - 1);
                if required_end_inclusive <= interval.end() {
                    Some(aligned_start)
                } else {
                    None
                }
            } else {
                None
            }
        })?;
        self.map
            .insert_merge_touching_if_values_equal(
                (aligned_start..=aligned_start + (S::SIZE - 1)).into(),
                MemoryType::UsedByKernel(crate::memory::KernelMemoryUsageType::PageTables),
            )
            .unwrap();
        Some(PhysFrame::from_start_address(PhysAddr::new(aligned_start)).unwrap())
    }
}
```
It's important that we use `insert_merge_touching_if_values_equal` to mark the now-allocated frame as used, so we don't allocate the same frame twice.

Let's update the `Memory` struct to use this wrapper type:
```rs
pub physical_memory: Spinlock<PhysicalMemory>,
```
And when we create `Memory`:
```rs
physical_memory: Spinlock::new(PhysicalMemory {
    map: physical_memory,
}),
```

## Mapping the frames
Now we can just use `PhysicalMemory` as a `FrameAllocator`:
```rs
for i in 0..n_pages {
    unsafe {
        offset_page_table
            .map_to(
                start_page + i,
                start_frame + i,
                PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                physical_memory.deref_mut(),
            )
            .unwrap()
            .flush();
    }
}
```
Finally, we return a `PhysicalMapping`, which tells the `acpi` crate about where we mapped the memory:
```rs
unsafe {
    PhysicalMapping::new(
        physical_address,
        NonNull::new(
            (start_page.start_address() + physical_address as u64 % S::SIZE).as_mut_ptr(),
        )
        .unwrap(),
        size,
        (n_pages * S::SIZE) as usize,
        self.clone(),
    )
}
```

## Implementing `unmap_physical_region_with_page_size`
Unmapping the physical region is pretty straightforward:
```rs
let level_4_table_physical_frame = Cr3::read().0;
let level_4_page_table = unsafe {
    VirtAddr::new(
        u64::from(region.handler().hhdm_offset)
            + level_4_table_physical_frame.start_address().as_u64(),
    )
    .as_mut_ptr::<PageTable>()
    .as_mut()
    .unwrap()
};
let mut offset_page_table = unsafe {
    OffsetPageTable::new(
        level_4_page_table,
        VirtAddr::new(region.handler().hhdm_offset.into()),
    )
};
let start_page =
    Page::<S>::containing_address(VirtAddr::new(region.virtual_start().as_ptr() as u64));
let n_pages = region.mapped_length() as u64 / S::SIZE;
for i in 0..n_pages {
    offset_page_table.unmap(start_page + i).unwrap().1.flush();
}
let _ = MEMORY.try_get().unwrap().used_virtual_memory.lock().cut({
    let start = start_page.start_address().as_u64();
    Interval::from(start..=start + (region.mapped_length() as u64 - 1))
});
```
We use the `cut` function to mark the virtual memory as usable again.

## Using our ACPI handler
In `acpi.rs`, add:
```rs
/// Safety: You can store the returned value in CPU local data, but you cannot send it across CPUs because the other CPUs did not flush their cache for changes in page tables
pub unsafe fn get_acpi_tables(
    rsdp: &RsdpResponse,
    hhdm_offset: HhdmOffset,
) -> AcpiTables<impl AcpiHandler> {
    let handler = KernelAcpiHandler { hhdm_offset };
    let address = rsdp.address();
    unsafe { AcpiTables::from_rsdp(handler, address) }.unwrap()
}
```
Then, in `main.rs`, after calling `memory::init`, add:
```rs
let rsdp = RSDP_REQUEST.get_response().unwrap();
// Safety: We're not sending this across CPUs
let acpi_tables = unsafe { acpi::get_acpi_tables(rsdp, hhdm_offset) }
    .headers()
    .map(|header| header.signature)
    .collect::<Box<[_]>>();
log::info!("ACPI Tables: {:?}", acpi_tables);
```
This should log:
```
[BSP] INFO  ACPI Tables: ["FACP", "APIC", "HPET", "WAET", "BGRT"]
```
We definitely will be using `APIC` and `HPET` later, so it's good that we are able to successfully parse those tables.

## ACPI tables on real hardware
### Jinlon
- FACP
- SSDT
- MCFG
- TPM2
- LPIT
- APIC
- SPCR
- DMAR
- DBG2
- HPET
- BGRT

### Lenovo Z560
- FACP
- ASF!
- HPET
- APIC
- MCFG
- SLIC
- BOOT
- ASPT
- WDRT
- SSDT
- SSDT
- SSDT

# Learn More
- https://wiki.osdev.org/RSDP
- https://wiki.osdev.org/ACPI
