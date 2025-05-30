use core::{fmt::Debug, ops::DerefMut, ptr::NonNull};

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use limine::response::RsdpResponse;
use nodit::{Interval, interval::iu};
use raw_cpuid::CpuId;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame, Size1GiB,
        Size2MiB,
    },
};

use crate::{hhdm_offset::HhdmOffset, memory::MEMORY};

/// Note: this cannot be sent across CPUs because the other CPUs did not flush their cache for changes in page tables
#[derive(Debug, Clone)]
struct KernelAcpiHandler {
    hhdm_offset: HhdmOffset,
}

impl KernelAcpiHandler {
    fn map_physical_region_with_page_size<S: PageSize + Debug, T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T>
    where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
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
                        physical_memory.deref_mut(),
                    )
                    .unwrap()
                    .flush();
            }
        }

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
    }

    fn unmap_physical_region_with_page_size<S: PageSize + Debug, T>(
        region: &acpi::PhysicalMapping<Self, T>,
    ) where
        for<'a> OffsetPageTable<'a>: Mapper<S>,
    {
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
    }
}

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

/// Safety: You can store the returned value in CPU local data, but you cannot send it across CPUs because the other CPUs did not flush their cache for changes in page tables
pub unsafe fn get_acpi_tables(
    rsdp: &RsdpResponse,
    hhdm_offset: HhdmOffset,
) -> AcpiTables<impl AcpiHandler> {
    let handler = KernelAcpiHandler { hhdm_offset };
    let address = rsdp.address();
    unsafe { AcpiTables::from_rsdp(handler, address) }.unwrap()
}
