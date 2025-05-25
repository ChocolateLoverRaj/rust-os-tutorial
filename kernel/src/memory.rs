use core::fmt::Debug;

use conquer_once::noblock::OnceCell;
use limine::{
    memory_map::EntryType,
    response::{ExecutableAddressResponse, ExecutableFileResponse, MemoryMapResponse},
};
use nodit::{Interval, NoditMap, NoditSet};
use spinning_top::Spinlock;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size1GiB, Size4KiB, Translate,
    },
};

use crate::hhdm_offset::HhdmOffset;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MemoryType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    ExecutableAndModules,
    FrameBuffer,
    UsedByKernel,
}

pub struct Memory {
    pub virtual_higher_half: Spinlock<NoditSet<u64, Interval<u64>>>,
    pub physical: Spinlock<PhysicalMemory>,
}

pub struct PhysicalMemory {
    pub map: NoditMap<u64, Interval<u64>, MemoryType>,
}

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
            MemoryType::UsedByKernel,
        );
        let phys_frame = PhysFrame::from_start_address(PhysAddr::new(aligned_start)).unwrap();
        // log::debug!("Allocating phys: {:?}", phys_frame);
        Some(phys_frame)
    }
}

pub static MEMORY: OnceCell<Memory> = OnceCell::uninit();

/// # Safety
/// This function must be called exactly once, before the page tables have been modified by the kernel
pub unsafe fn init(
    memory_map: &'static MemoryMapResponse,
    hhdm_offset: HhdmOffset,
    executable_address: &'static ExecutableAddressResponse,
    executable_file: &'static ExecutableFileResponse,
) {
    let mut physical = PhysicalMemory {
        map: {
            let mut physical_memory = NoditMap::<u64, Interval<_>, _>::new();
            for entry in memory_map.entries() {
                physical_memory
                    .insert_merge_touching_if_values_equal(
                        (entry.base..entry.base + entry.length).into(),
                        match entry.entry_type {
                            EntryType::USABLE => MemoryType::Usable,
                            EntryType::RESERVED => MemoryType::Reserved,
                            EntryType::ACPI_RECLAIMABLE => MemoryType::AcpiReclaimable,
                            EntryType::ACPI_NVS => MemoryType::AcpiNvs,
                            EntryType::BAD_MEMORY => MemoryType::BadMemory,
                            EntryType::BOOTLOADER_RECLAIMABLE => MemoryType::BootloaderReclaimable,
                            EntryType::EXECUTABLE_AND_MODULES => MemoryType::ExecutableAndModules,
                            EntryType::FRAMEBUFFER => MemoryType::FrameBuffer,
                            _ => unreachable!(),
                        },
                    )
                    .unwrap();
            }
            log::debug!("Physical memory: {:#X?}", physical_memory);
            physical_memory
        },
    };

    // Create a new tables, because the old one has unused mappings
    let new_l4_frame: PhysFrame<Size4KiB> = physical.allocate_frame().unwrap();
    let new_l4_page_table = unsafe {
        VirtAddr::new(u64::from(hhdm_offset) + new_l4_frame.start_address().as_u64())
            .as_mut_ptr::<PageTable>()
            .as_uninit_mut()
            .unwrap()
            .write(Default::default())
    };
    let mut new_offset_page_table =
        unsafe { OffsetPageTable::new(new_l4_page_table, VirtAddr::new(hhdm_offset.into())) };
    let mut used_virtual_memory = NoditSet::<u64, Interval<_>>::new();
    let level_4_table_physical_frame = Cr3::read().0;
    let level_4_page_table = unsafe {
        VirtAddr::new(
            u64::from(hhdm_offset) + level_4_table_physical_frame.start_address().as_u64(),
        )
        .as_mut_ptr::<PageTable>()
        .as_mut()
        .unwrap()
    };
    let offset_page_table =
        unsafe { OffsetPageTable::new(level_4_page_table, VirtAddr::new(hhdm_offset.into())) };

    // Add everything that's HHDM mapped
    let ref mut last_mapped_addr = None;
    for entry in memory_map.entries() {
        if [
            EntryType::USABLE,
            EntryType::BOOTLOADER_RECLAIMABLE,
            EntryType::EXECUTABLE_AND_MODULES,
            EntryType::FRAMEBUFFER,
        ]
        .contains(&entry.entry_type)
        {
            type S = Size1GiB;
            let physical_start = PhysAddr::new(entry.base);
            let virtual_start = VirtAddr::new(entry.base + u64::from(hhdm_offset));
            let len = entry.length;
            let first_frame = PhysFrame::<S>::containing_address(physical_start);
            let frame_count = (physical_start.as_u64() + (len - 1)) / S::SIZE
                - physical_start.as_u64() / S::SIZE
                + 1;
            let first_page = Page::<S>::containing_address(virtual_start);
            for i in 0..frame_count {
                let frame = first_frame + i;
                let page = first_page + i;
                if last_mapped_addr
                    .as_mut()
                    .is_none_or(|last_mapped_phys_addr| {
                        frame.start_address() > *last_mapped_phys_addr
                    })
                {
                    unsafe {
                        new_offset_page_table.map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut physical,
                        )
                    }
                    .unwrap()
                    .ignore();
                    *last_mapped_addr = Some(frame.start_address() + (S::SIZE - 1));
                }
            }
            used_virtual_memory.insert_merge_touching_or_overlapping({
                (first_page.start_address().as_u64()
                    ..=(first_page + (frame_count - 1)).start_address().as_u64() + (S::SIZE - 1))
                    .into()
            });
        }
    }

    // Add the topmost 2 GiB (in 48-bit virtual address space)
    {
        let start_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            executable_address.physical_base(),
        ));
        let start_page =
            Page::<Size4KiB>::containing_address(VirtAddr::new(executable_address.virtual_base()));
        let last_page = Page::<Size4KiB>::containing_address(VirtAddr::new(
            executable_address.virtual_base() + (executable_file.file().size() - 1),
        ));
        let page_count = last_page - start_page + 1;
        for i in 0..page_count {
            unsafe {
                new_offset_page_table
                    .map_to(
                        start_page + i,
                        start_frame + i,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut physical,
                    )
                    .unwrap()
                    .flush()
            };
        }
    }
    log::debug!("Used virtual address ranges: {:#X?}", used_virtual_memory);

    let current_translation = offset_page_table
        .translate_addr(VirtAddr::new(0xffff800003bc6330))
        .unwrap();
    let new_translation = new_offset_page_table
        .translate_addr(VirtAddr::new(0xffff800003bc6330))
        .unwrap();
    assert_eq!(current_translation, new_translation);

    // Switch Cr3
    log::info!("New l4 frame: {:?}", new_l4_frame);
    unsafe { Cr3::write(new_l4_frame, Cr3::read().1) };

    MEMORY
        .try_init_once(|| Memory {
            virtual_higher_half: Spinlock::new(used_virtual_memory),
            physical: Spinlock::new(physical),
        })
        .unwrap();
}
