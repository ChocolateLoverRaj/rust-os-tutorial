use core::{fmt::Debug, ptr::NonNull};

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use common::AllocPageSize;
use limine::response::RsdpResponse;
use raw_cpuid::CpuId;
use x86_64::{PhysAddr, VirtAddr, structures::paging::PageTableFlags};

use crate::memory::MEMORY;

/// Note: this cannot be sent across CPUs because the other CPUs did not flush their cache for changes in page tables
#[derive(Debug, Clone)]
struct KernelAcpiHandler;

fn max_page_size() -> AllocPageSize {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .is_some_and(|info| info.has_1gib_pages())
    {
        AllocPageSize::_1GiB
    } else {
        AllocPageSize::_2MiB
    }
}

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let page_size = max_page_size();
        let memory = MEMORY.get().unwrap();
        let mut physical_memory = memory.physical_memory.lock();
        let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
        let mut virtual_memory = memory.virtual_memory.lock();

        let n_pages = ((size + physical_address) as u64).div_ceil(page_size.byte_len_u64())
            - physical_address as u64 / page_size.byte_len_u64();
        let start_frame = PhysAddr::new(
            physical_address as u64 / page_size.byte_len_u64() * page_size.byte_len_u64(),
        );
        let mut pages = virtual_memory
            .allocate_contiguous_pages_2(page_size, n_pages)
            .unwrap();
        let start_page = VirtAddr::new(*pages.range().start());

        for i in 0..n_pages {
            unsafe {
                pages.map_to(
                    start_page + i * page_size.byte_len_u64(),
                    start_frame + i * page_size.byte_len_u64(),
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE | PageTableFlags::GLOBAL,
                    &mut frame_allocator,
                );
            }
        }

        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new(
                    (start_page + physical_address as u64 % page_size.byte_len_u64()).as_mut_ptr(),
                )
                .unwrap(),
                size,
                n_pages as usize * page_size.byte_len(),
                self.clone(),
            )
        }
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        let page_size = max_page_size();
        let start_page = (region.virtual_start().as_ptr() as u64) / page_size.byte_len_u64()
            * page_size.byte_len_u64();
        let mut virtual_memory = MEMORY.get().unwrap().virtual_memory.lock();
        let range = start_page..=start_page + region.mapped_length() as u64 - 1;
        // Safety: this function will only be called with regions mapped by the `map_physical_region` function
        unsafe { virtual_memory.already_allocated_2(page_size, range) }.unmap_and_deallocate();
    }
}

/// # Safety
/// You can store the returned value in CPU local data, but you cannot send it across CPUs because the other CPUs did not flush their cache for changes in page tables
pub unsafe fn get_acpi_tables(rsdp: &RsdpResponse) -> AcpiTables<impl AcpiHandler> {
    let address = rsdp.address();
    unsafe { AcpiTables::from_rsdp(KernelAcpiHandler, address) }.unwrap()
}
