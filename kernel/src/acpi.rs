use core::{fmt::Debug, ptr::NonNull};

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use ez_paging::{ConfigurableFlags, Frame, Page};
use limine::response::RsdpResponse;
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::{max_page_size, memory::MEMORY};

/// Note: this cannot be sent across CPUs because the other CPUs did not flush their cache for changes in page tables
#[derive(Debug, Clone)]
struct KernelAcpiHandler;

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
        let start_frame = Frame::new(
            PhysAddr::new(
                physical_address as u64 / page_size.byte_len_u64() * page_size.byte_len_u64(),
            ),
            page_size,
        )
        .unwrap();
        let mut pages = virtual_memory
            .allocate_contiguous_pages(page_size, n_pages)
            .unwrap();
        let start_page = Page::new(VirtAddr::new(*pages.range().start()), page_size).unwrap();

        for i in 0..n_pages {
            let page = start_page.offset(i).unwrap();
            let frame = start_frame.offset(i).unwrap();
            let flags = ConfigurableFlags {
                executable: false,
                writable: false,
                pat_memory_type: PatMemoryType::WriteBack,
            };
            unsafe {
                pages
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .unwrap();
            }
        }

        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new(
                    (start_page.start_addr() + physical_address as u64 % page_size.byte_len_u64())
                        .as_mut_ptr(),
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
        unsafe { virtual_memory.already_allocated(page_size, range) }.unmap_and_deallocate();
    }
}

/// # Safety
/// You can store the returned value in CPU local data, but you cannot send it across CPUs because the other CPUs did not flush their cache for changes in page tables
pub unsafe fn get_acpi_tables(rsdp: &RsdpResponse) -> AcpiTables<impl AcpiHandler> {
    let address = rsdp.address();
    unsafe { AcpiTables::from_rsdp(KernelAcpiHandler, address) }.unwrap()
}
