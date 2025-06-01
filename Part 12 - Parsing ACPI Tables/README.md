# Parsing ACPI Tables
ACPI tables is binary data that provides information about the computer to the operating system. We'll need to parse ACPI tables to send interrupts between CPUs, access timers, and more. We'll use the `acpi` crate, which parses the binary data into nice Rust types.
```toml
acpi = "5.2.0"
```
Create a file `acpi.rs`. We'll be using the [`AcpiTables::from_rsdp`](https://docs.rs/acpi/5.2.0/acpi/struct.AcpiTables.html#method.from_rsdp) method. It needs a handler, which maps the ACPI memory, and the address of the [RSDP](https://wiki.osdev.org/RSDP). 

## RSDP request
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

## Setting up our own page tables
We're going to be mapping virtual memory to physical memory. Currently, we don't really know which parts in virtual memory are already mapped. It could cause issues if we try to map a page which is already mapped. So we'll create a new, blank L4 page table. That way, we know exactly what should and shouldn't be used. However, we need to re-create the mappings that Limine made.

We will need to know which physical memory regions are available to use to set up page tables. We will need to know which virtual memory regions are available to map pages.

### Physical memory
To keep track of which regions in memory are used for what, we will use the `nodit` crate:
```toml
nodit = "0.9.2"
```
It lets us create a map where the keys are *ranges*, and handles automatically merging ranges same values when they touch.

Create a file `memory/physical_memory.rs`. First, let's create some enums:
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

Then let's create a wrapper type around the nodit map:
```rs
pub struct PhysicalMemory {
    pub(super) map: NoditMap<u64, Interval<u64>, MemoryType>,
}
```
We use `pub(super)` so that `memory.rs` can modify the map, but other code can't.

Then, in `memory.rs`, after initializing the global allocator, let's make sure to drop our mutex guard so that our allocator isn't locked while we try to allocate:
```rs
// Make sure to drop the mutex guard so that we can allocate without a deadlock
{
    let mut talc = GLOBAL_ALLOCATOR.lock();
    let span = global_allocator_mem.into();
    // Safety: We got the span from valid memory
    unsafe { talc.claim(span) }.unwrap();
}
```
Then let's create a `PhysicalMemory`, handling the initial memory from Limine and counting the memory we used for our global allocator:
```rs
let mut physical_memory = PhysicalMemory {
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
        let _ = map.insert_overwrite(
            (global_allocator_physical_start
                ..=global_allocator_physical_start + (global_allocator_size - 1))
                .into(),
            MemoryType::UsedByKernel(KernelMemoryUsageType::GlobalAllocatorHeap),
        );
        map
    },
};
```

### Implementing `FrameAllocator` for physical memory
To start creating new page tables, we'll need a `FrameAllocator`. We can implement it for our `PhysicalMemory` struct:
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
        let _ = self.map.insert_overwrite(
            (aligned_start..=aligned_start + (S::SIZE - 1)).into(),
            MemoryType::UsedByKernel(KernelMemoryUsageType::PageTables),
        );
        Some(PhysFrame::from_start_address(PhysAddr::new(aligned_start)).unwrap())
    }
}
```
Then we can use it to create a new level 4 page table:
```rs
let new_l4_frame = FrameAllocator::<Size4KiB>::allocate_frame(&mut physical_memory).unwrap();
// Safety: The allocated frame is in usable memory, which is offset mapped
let new_l4_page_table =
    VirtAddr::new(u64::from(hhdm_offset) + new_l4_frame.start_address().as_u64())
        // We use `MaybeUninit` because the memory is uninitialized
        .as_mut_ptr::<MaybeUninit<PageTable>>();
let new_l4_page_table = unsafe {
    new_l4_page_table
        .as_mut()
        .unwrap()
        // We initialize the page table to be blank
        .write(Default::default())
};
// Safety: We are only using usable memory, which is offset mapped
let mut new_offset_page_table =
    unsafe { OffsetPageTable::new(new_l4_page_table, VirtAddr::new(hhdm_offset.into())) };
```

### Detecting support for 1 GiB pages
Next, we need to offset map everything that is already offset mapped by Limine. Ideally, we should use 1 GiB pages. However some old computers (including my Lenovo Z560) don't support 1 GiB pages. In this case, we need to fall back to using 2 MiB pages. We'll use the `raw-cpuid` crate to detect support for 1 GiB pages:
```toml
raw-cpuid = "11.5.0"
```
Let's move everything in our `init` function in `memory.rs` into a generic function:
```rs
fn init_with_page_size<S: PageSize + Debug>(
    memory_map: &'static MemoryMapResponse,
    hhdm_offset: HhdmOffset,
) where
    for<'a> OffsetPageTable<'a>: Mapper<S>,
{}
```
And then in the `init` function:
```rs
if CpuId::new()
    .get_extended_processor_and_feature_identifiers()
    .unwrap()
    .has_1gib_pages()
{
    init_with_page_size::<Size1GiB>(memory_map, hhdm_offset);
} else {
    init_with_page_size::<Size2MiB>(memory_map, hhdm_offset);
}
```

### Offset mapping
Then, in `init_with_page_size`, we can offset map entries using `S`:
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
                            &mut physical_memory,
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
```

### Finishing up the new page tables
Finally we reuse the mappings for the top 512 GiB of memory, because the kernel lies in the top 2 GiB and mapping it is not simply a matter of offset mapping large pages.
```rs
// We must map the kernel, which lies in the top 2 GiB of virtual memory
// We can just reuse Limine's mappings for the top 512 GiB
let (current_l4_frame, cr3_flags) = Cr3::read();
let current_l4_page_table = unsafe {
    VirtAddr::new(u64::from(hhdm_offset) + current_l4_frame.start_address().as_u64())
        .as_mut_ptr::<PageTable>()
        .as_mut()
        .unwrap()
};
new_l4_page_table[511].clone_from(&current_l4_page_table[511]);
```
Now we can switch to the new page tables without breaking anything:
```rs
// Safety: Everything that needs to be mapped is mapped
unsafe { Cr3::write(new_l4_frame, cr3_flags) };
```

### Keeping track of virtual memory
Create a file `memory/virtual_memory.rs`. In it, we will have a wrapper struct similar to the physical memory one.
```rs
pub struct VirtualMemory {
    pub(super) set: NoditSet<u64, Interval<u64>>,
    pub(super) cr3: PhysFrame<Size4KiB>,
    pub(super) hhdm_offset: HhdmOffset,
}
```
We use `NoditSet` because we just need to keep track of if memory is used or not. We don't really care what it's used for.

Back in `init_with_page_size`, we can create the initial struct:
```rs
let virtual_memory = VirtualMemory {
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
                let start = u64::from(hhdm_offset) + entry.base / S::SIZE * S::SIZE;
                let end = u64::from(hhdm_offset)
                    + (entry.base + (entry.length - 1)) / S::SIZE * S::SIZE
                    + (S::SIZE - 1);
                set.insert_merge_touching_or_overlapping((start..=end).into());
            }
        }
        // Let's add the top 512 GiB
        set.insert_merge_touching(iu(0xFFFFFF8000000000)).unwrap();
        set
    },
    cr3: new_l4_frame,
    hhdm_offset,
};
```

### Storing the memory info
In `memory.rs`, create a struct to store the output from the init function:
```rs
#[non_exhaustive]
pub struct Memory {
    pub physical_memory: spin::Mutex<PhysicalMemory>,
    pub virtual_memory: spin::Mutex<VirtualMemory>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}
```
The `#[non_exhaustive]` makes it so that we can't accidentally construct a `Memory` from outside `memory.rs`. Then create a global variable for it:
```rs
pub static MEMORY: Once<Memory> = Once::new();
```
And then at the end of `init_with_page_size`, set it:
```rs
MEMORY.call_once(|| Memory {
    physical_memory: Mutex::new(physical_memory),
    virtual_memory: Mutex::new(virtual_memory),
    new_kernel_cr3: new_l4_frame,
    new_kernel_cr3_flags: cr3_flags,
});
```

### Switching Cr3 on other CPUs
Every CPU has its own Cr3 register (just like other registers), so we need to switch Cr3 for the APs, not just the BSP. In the top of `entry_point_from_limine_mp`, add:
```rs
let memory = MEMORY.get().unwrap();
// Safety: The Cr3 and flags is valid
unsafe {
    Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags);
}
```

## ACPI handler generic methods
Our ACPI handler will also be mapping and un-mapping pages with the page size determined at run time. Let's create generic internal methods:
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

## Allocating virtual memory
Now that we have a `VirtualMemory` struct, the process for modifying page tables is:
- Lock the `VirtualMemory`. This also ensures that the page tables aren't modified concurrently.
- Find unused virtual memory.
- Mark the virtual memory you will use as used.
- Map pages

In `memory/virtual_memory.rs`, let's create methods that will make it easy to claim virtual memory and also prevent accidentally modifying the wrong pages:
```rs
impl VirtualMemory {
    /// Returns the start page of the allocated range of pages.
    /// Pages are guaranteed not to be mapped.
    pub fn allocate_contiguous_pages<S: PageSize + Debug>(
        &mut self,
        n_pages: u64,
    ) -> Option<AllocatedPages<S>> {
        let start_page = Page::<S>::from_start_address(VirtAddr::new({
            let range = self
                .set
                .gaps_trimmed(iu(0xffff800000000000))
                .find_map(|gap| {
                    let aligned_start = gap.start().next_multiple_of(S::SIZE);
                    let required_end_inclusive = aligned_start + (n_pages * S::SIZE - 1);
                    if required_end_inclusive <= gap.end() {
                        Some(aligned_start..=required_end_inclusive)
                    } else {
                        None
                    }
                })?;
            let start = *range.start();
            self.set
                .insert_merge_touching(Interval::from(range))
                .unwrap();
            start
        }))
        .unwrap();
        Some(AllocatedPages {
            virtual_memory: self,
            range: start_page..=start_page + (n_pages - 1),
        })
    }

    /// # Safety
    /// The pages must have been allocated by [`VirtualMemory`]
    pub unsafe fn already_allocated<S: PageSize>(
        &mut self,
        pages: RangeInclusive<Page<S>>,
    ) -> AllocatedPages<'_, S> {
        AllocatedPages {
            virtual_memory: self,
            range: pages,
        }
    }
}

pub struct AllocatedPages<'a, S: PageSize> {
    virtual_memory: &'a mut VirtualMemory,
    range: RangeInclusive<Page<S>>,
}

impl<S: PageSize> AllocatedPages<'_, S> {
    pub fn range(&self) -> &RangeInclusive<Page<S>> {
        &self.range
    }

    fn get_offset_page_table(&self) -> OffsetPageTable<'_> {
        let level_4_page_table = VirtAddr::new(
            u64::from(self.virtual_memory.hhdm_offset)
                + self.virtual_memory.cr3.start_address().as_u64(),
        )
        .as_mut_ptr::<PageTable>();
        // Safety: We can access it through HHDM
        let level_4_page_table = unsafe { level_4_page_table.as_mut() }.unwrap();
        // Safety: No other code is currently modifying page tables
        unsafe {
            OffsetPageTable::new(
                level_4_page_table,
                VirtAddr::new(self.virtual_memory.hhdm_offset.into()),
            )
        }
    }

    /// # Safety
    /// See the safety for [`x86_64::structures::paging::mapper::Mapper::map_to`]
    pub unsafe fn map_to(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) where
        S: Debug,
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        if self.range.contains(&page) {
            let mut offset_page_table = self.get_offset_page_table();
            // Safety: same as this function's safety, plus we ensure that the page we are mapping is allocated properly
            unsafe { offset_page_table.map_to(page, frame, flags, frame_allocator) }
                .unwrap()
                .flush();
        } else {
            panic!(
                "Tried to map page {page:?}, which is outside of allocated range {:?}",
                self.range
            )
        }
    }

    /// All pages must be mapped
    pub fn unmap_and_deallocate(self)
    where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
        let pages = self.range.clone();
        let mut offset_page_table = self.get_offset_page_table();
        for page in pages.clone() {
            offset_page_table.unmap(page).unwrap().1.flush();
        }
        let _ = self.virtual_memory.set.cut({
            let start = pages.start().start_address().as_u64();
            let end_inclusive = pages.end().start_address().as_u64() + (S::SIZE - 1);
            Interval::from(start..=end_inclusive)
        });
    }
}
```
This will make our ACPI handler code very simple.

## Mapping the frames
In our `map_physical_region_with_page_size` function, first we lock the physical and virtual memory:
```rs
let memory = MEMORY.get().unwrap();
let mut physical_memory = memory.physical_memory.lock();
let mut virtual_memory = memory.virtual_memory.lock();
```
Then we allocate pages:
```rs
let n_pages = ((size + physical_address) as u64).div_ceil(S::SIZE)
    - physical_address as u64 / S::SIZE;
let start_frame =
    PhysFrame::<S>::containing_address(PhysAddr::new(physical_address as u64));
let mut pages = virtual_memory.allocate_contiguous_pages(n_pages).unwrap();
```
Then we map the pages:
```rs
let start_page = *pages.range().start();

for i in 0..n_pages {
    unsafe {
        pages.map_to(
            start_page + i,
            start_frame + i,
            PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
            physical_memory.deref_mut(),
        );
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
Un-mapping the physical region is pretty straightforward:
```rs
let start_page =
    Page::<S>::containing_address(VirtAddr::new(region.virtual_start().as_ptr() as u64));
let n_pages = region.mapped_length() as u64 / S::SIZE;
let mut virtual_memory = MEMORY.get().unwrap().virtual_memory.lock();
let pages = start_page..=start_page + (n_pages - 1);
// Safety: this function will only be called with regions mapped by the `map_physical_region` function
unsafe { virtual_memory.already_allocated(pages) }.unmap_and_deallocate();
```

## Using our ACPI handler
In `acpi.rs`, add:
```rs
/// # Safety
/// You can store the returned value in CPU local data, but you cannot send it across CPUs because the other CPUs did not flush their cache for changes in page tables
pub unsafe fn get_acpi_tables(rsdp: &RsdpResponse) -> AcpiTables<impl AcpiHandler> {
    let address = rsdp.address();
    unsafe { AcpiTables::from_rsdp(KernelAcpiHandler, address) }.unwrap()
}
```
Then, in `main.rs`, after calling `memory::init`, add:
```rs
let rsdp = RSDP_REQUEST.get_response().unwrap();
// Safety: We're not sending this across CPUs
let acpi_tables = unsafe { acpi::get_acpi_tables(rsdp) }
    .headers()
    .map(|header| header.signature)
    .collect::<Box<[_]>>();
log::info!("ACPI Tables: {acpi_tables:?}");
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
