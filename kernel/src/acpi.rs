use core::{fmt::Debug, ops::DerefMut, ptr::NonNull};

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use limine::response::RsdpResponse;
use nodit::{Interval, interval::iu};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame, Size1GiB,
    },
};

use crate::{hhdm_offset::HhdmOffset, memory::MEMORY};

#[derive(Debug, Clone)]
struct KernelAcpiHandler {
    hhdm_offset: HhdmOffset,
}

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let memory = MEMORY.try_get().unwrap();
        let mut physical = memory.physical.lock();
        let mut virtual_higher_half = memory.virtual_higher_half.lock();

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
        // log::debug!(
        //     "Mapping {} {} frames starting from phys: {:?}, virt: {:?}",
        //     n_pages,
        //     Size1GiB::DEBUG_STR,
        //     start_frame,
        //     start_page
        // );

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
        for i in 0..n_pages {
            unsafe {
                offset_page_table
                    .map_to(
                        start_page + i,
                        start_frame + i,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                        physical.deref_mut(),
                    )
                    .unwrap()
                    .flush();
            }
        }

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
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
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
        // log::debug!("Unmapping {} starting at {:?}", n_pages, start_page);
        for i in 0..n_pages {
            offset_page_table.unmap(start_page + i).unwrap().1.flush();
        }
        let _ = MEMORY.try_get().unwrap().virtual_higher_half.lock().cut({
            let start = start_page.start_address().as_u64();
            Interval::from(start..=start + (region.mapped_length() as u64 - 1))
        });
    }
}

pub fn init(rsdp: &RsdpResponse, hhdm_offset: HhdmOffset) {
    let handler = KernelAcpiHandler { hhdm_offset };
    let address = rsdp.address();
    let acpi_tables = unsafe { AcpiTables::from_rsdp(handler, address) }.unwrap();
    acpi_tables
        .headers()
        .for_each(|header| log::info!("ACPI Table: {:#?}", header.signature));
}
