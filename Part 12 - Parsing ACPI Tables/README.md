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
when creating `Memory`. Let's mark all of the offset mapped region as used:
```rs
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
        // Remember to canonicalize higher-half addresses
        let start = VirtAddr::new_truncate(
            u64::from(hhdm_offset) + entry.base / Size1GiB::SIZE * Size1GiB::SIZE,
        )
        .as_u64();
        let end = start + (Size1GiB::SIZE - 1);
        used_virtual_memory.insert_merge_touching_or_overlapping((start..=end).into());
    }
}
```
We are also reserving the top 512 GiB since we are reusing Limine's L3 page table:
```rs
// Let's add the top 512 GiB
used_virtual_memory
    .insert_merge_touching(iu(0xFFFFFF8000000000))
    .unwrap();
``` 
All other virtual memory is available and unmapped in our new page tables.

## Finding unused virtual memory
In our `map_physical_region` method, let's lock the physical and virtual memory, and find a contiguous region of virtual memory in the higher half, made up of 1 GiB pages: 
```rs
let memory = MEMORY.try_get().unwrap();
let mut physical_memory = memory.physical_memory.lock();
let mut virtual_higher_half = memory.used_virtual_memory.lock();

let n_pages = (((size + physical_address) as u64).div_ceil(Size1GiB::SIZE)
    - physical_address as u64 / Size1GiB::SIZE) as u64;
let start_frame =
    PhysFrame::<Size1GiB>::containing_address(PhysAddr::new(physical_address as u64));
let start_page = Page::<Size1GiB>::from_start_address(VirtAddr::new({
    let range = virtual_higher_half
        .gaps_trimmed(iu(0xffff800000000000))
        .find_map(|gap| {
            let aligned_start = gap.start().next_multiple_of(Size1GiB::SIZE);
            let required_end_inclusive = aligned_start + (n_pages * Size1GiB::SIZE - 1);
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
            (start_page.start_address() + physical_address as u64 % Size1GiB::SIZE)
                .as_mut_ptr(),
        )
        .unwrap(),
        size,
        (n_pages * Size1GiB::SIZE) as usize,
        self.clone(),
    )
}
```

## Implementing `unmap_physical_region`
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
let start_page = Page::<Size1GiB>::containing_address(VirtAddr::new(
    region.virtual_start().as_ptr() as u64,
));
let n_pages = region.mapped_length() as u64 / Size1GiB::SIZE;
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
unsafe {
    acpi::get_acpi_tables(rsdp, hhdm_offset)
        .headers()
        .for_each(|header| log::info!("ACPI Table: {:#?}", header.signature))
};
```
This should log:
```
[BSP] INFO  ACPI Table: "FACP"
[BSP] INFO  ACPI Table: "APIC"
[BSP] INFO  ACPI Table: "HPET"
[BSP] INFO  ACPI Table: "WAET"
[BSP] INFO  ACPI Table: "BGRT"
```
We definitely will be using `APIC` and `HPET` later, so it's good that we are able to successfully parse those tables.

# Learn More
- https://wiki.osdev.org/RSDP
- https://wiki.osdev.org/ACPI
