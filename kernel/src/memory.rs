use core::mem::MaybeUninit;

use conquer_once::noblock::OnceCell;
use limine::{memory_map::EntryType, response::MemoryMapResponse};
use linked_list_allocator::LockedHeap;
use nodit::{Interval, NoditMap};
use spinning_top::Spinlock;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size1GiB, Size4KiB,
    },
};

use crate::{hhdm_offset::HhdmOffset, initial_frame_allocator::InitialFrameAllocator};

#[global_allocator]
static GLOBAL_ALLOCATOR: LockedHeap = LockedHeap::empty();

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

pub struct Memory {
    pub physical_memory: Spinlock<NoditMap<u64, Interval<u64>, MemoryType>>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}

pub static MEMORY: OnceCell<Memory> = OnceCell::uninit();

/// Sets up a new L4 page table, initializes the global allocator, switches Cr3 to the new page table, and initializes `MEMORY`
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
    unsafe {
        GLOBAL_ALLOCATOR.lock().init(
            VirtAddr::new(u64::from(hhdm_offset) + global_allocator_physical_start).as_mut_ptr(),
            global_allocator_size as usize,
        )
    };

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

    log::debug!("Physical memory usage: {:#X?}", physical_memory);

    MEMORY
        .try_init_once(|| Memory {
            physical_memory: Spinlock::new(physical_memory),
            new_kernel_cr3: new_l4_frame,
            new_kernel_cr3_flags: cr3_flags,
        })
        .unwrap();
}
