use core::{fmt::Debug, mem::MaybeUninit, slice};

use limine::{memory_map::EntryType, response::MemoryMapResponse};
use nodit::{NoditMap, NoditSet, interval::iu};
pub use physical_memory::{
    KernelMemoryUsageType, MemoryType, PhysicalMemory, UserModeMemoryUsageType,
};
use raw_cpuid::CpuId;
use spin::{Mutex, Once};
use talc::{ErrOnOom, Talc, Talck};
use virtual_memory::VirtualMemory;
use x86_64::{
    PhysAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, PageSize, PageTableFlags, PhysFrame, Size1GiB,
        Size2MiB, Size4KiB,
    },
};

use crate::{
    get_page_table::get_page_table,
    hhdm_offset::HhdmOffset,
    translate_addr::{TranslateAddr, TranslateFrame},
};

mod physical_memory;
mod virtual_memory;

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

#[non_exhaustive]
pub struct Memory {
    pub physical_memory: spin::Mutex<PhysicalMemory>,
    pub virtual_memory: spin::Mutex<VirtualMemory>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}

pub static MEMORY: Once<Memory> = Once::new();

fn init_with_page_size<S: PageSize + Debug>(memory_map: &'static MemoryMapResponse)
where
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

    let ptr = PhysAddr::new(global_allocator_physical_start)
        .to_virt()
        .as_mut_ptr();
    // Safety: We've reserved the physical memory and it is already offset mapped
    let global_allocator_mem = unsafe {
        slice::from_raw_parts_mut::<MaybeUninit<u8>>(ptr, global_allocator_size as usize)
    };
    // Make sure to drop the mutex guard so that we can allocate without a deadlock
    {
        let mut talc = GLOBAL_ALLOCATOR.lock();
        let span = global_allocator_mem.into();
        // Safety: We got the span from valid memory
        unsafe { talc.claim(span) }.unwrap();
    }

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
    let mut frame_allocator = physical_memory.get_kernel_frame_allocator();

    let new_l4_frame = FrameAllocator::<Size4KiB>::allocate_frame(&mut frame_allocator).unwrap();
    // Safety: The allocated frame is in usable memory, which is offset mapped
    let mut new_l4_page_table = unsafe { get_page_table(new_l4_frame, true) };

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
                    let page = frame.to_page();
                    unsafe {
                        new_l4_page_table
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

    // We must map the kernel, which lies in the top 2 GiB of virtual memory
    // We can just reuse Limine's mappings for the top 512 GiB
    let (current_l4_frame, cr3_flags) = Cr3::read();
    let current_page_table = unsafe { get_page_table(current_l4_frame, false) };
    new_l4_page_table.level_4_table_mut()[511].clone_from(&current_page_table.level_4_table()[511]);

    // Safety: Everything that needs to be mapped is mapped
    unsafe { Cr3::write(new_l4_frame, cr3_flags) };

    let virtual_memory = VirtualMemory {
        set: {
            let hhdm_offset = HhdmOffset::get_from_response();
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
    };

    MEMORY.call_once(|| Memory {
        physical_memory: Mutex::new(physical_memory),
        virtual_memory: Mutex::new(virtual_memory),
        new_kernel_cr3: new_l4_frame,
        new_kernel_cr3_flags: cr3_flags,
    });
}

/// Finds unused physical memory for the global allocator and initializes the global allocator
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init(memory_map: &'static MemoryMapResponse) {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .unwrap()
        .has_1gib_pages()
    {
        init_with_page_size::<Size1GiB>(memory_map);
    } else {
        init_with_page_size::<Size2MiB>(memory_map);
    }
}
