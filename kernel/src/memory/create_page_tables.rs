use core::{ops::Deref, ptr::NonNull};

use ez_paging::{ConfigurableFlags, Frame, ManagedPat, PagingConfig, max_page_size};
use limine::{memory_map::EntryType, response::MemoryMapResponse};
use nodit::{NoditSet, interval::iu};
use x86_64::{
    PhysAddr,
    registers::{
        control::{Cr3, Cr3Flags},
        model_specific::PatMemoryType,
    },
    structures::paging::{PageTable, PhysFrame},
};

use crate::{
    hhdm_offset::hhdm_offset,
    translate_addr::{OffsetMappedPhysAddr, OffsetMappedPhysFrame},
};

use super::{physical_memory::PhysicalMemory, virtual_memory::VirtualMemory};

/// Creates new page tables, but does not switch to them
pub fn create_page_tables(
    memory_map: &'static MemoryMapResponse,
    physical_memory: &mut PhysicalMemory,
) -> (PhysFrame, Cr3Flags, VirtualMemory) {
    let hhdm_offset = hhdm_offset();
    let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
    let mut l4 = PagingConfig::new(
        // Safety: we don't touch the PAT
        unsafe { ManagedPat::new() },
        hhdm_offset.into(),
    )
    .new_kernel(frame_allocator.allocate_4kib_frame().unwrap());

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

    // We must map the kernel, which lies in the top 2 GiB of virtual memory
    // We can just reuse Limine's mappings for the top 512 GiB
    let (current_l4_frame, cr3_flags) = Cr3::read();
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
    let new_l4_page_table = {
        let mut ptr = l4.page_table();
        // Safety: we are just going to copy the last entry, and not modify that region's mappings
        unsafe { ptr.as_mut() }
    };
    new_l4_page_table[511].clone_from(&current_l4_page_table[511]);

    (
        *l4.frame().deref(),
        cr3_flags,
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
        },
    )
}
