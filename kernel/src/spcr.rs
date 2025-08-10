use acpi::{
    AcpiHandler, AcpiTables,
    address::AddressSpace,
    spcr::{Spcr, SpcrInterfaceType},
};
use alloc::boxed::Box;
use uart::{address::MmioAddress, writer::UartWriter};
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};

use crate::{
    ConfigurableFlags, Frame, Page,
    logger::{self, AnyWriter},
    max_page_size,
    memory::MEMORY,
};

/// Checks for SPCR, and sets logger to log through SPCR instead of COM1 accordingly
pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    let page_size = max_page_size();
    if let Some(uart) = acpi_tables
        .find_table::<Spcr>()
        // The table might not exist
        .ok()
        .and_then(|spcr| {
            // We may not know how to handle the interface type
            match spcr.interface_type() {
                // These 3 can be handled by the uart crate
                SpcrInterfaceType::Full16550
                | SpcrInterfaceType::Full16450
                | SpcrInterfaceType::Generic16550 => spcr.base_address(),
                _ => None,
            }
        })
        // We get the base address, which is how we access the uart
        .and_then(|base_address| base_address.ok())
        // https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#generic-address-structure-gas
        // ACPI addresses can be many different types. We will only handle system memory (MMIO)
        .filter(|base_address| base_address.address_space == AddressSpace::SystemMemory)
        .filter(|base_address| {
            base_address.bit_offset == 0 && base_address.bit_width.is_multiple_of(8)
        })
        .map(|base_address| {
            let stride_bytes = base_address.bit_width / 8;
            let memory = MEMORY.get().unwrap();
            let phys_start_address = base_address.address;
            let len = u64::from(stride_bytes) * 8;
            let start_frame = Frame::new(
                PhysAddr::new(phys_start_address).align_down(page_size.byte_len_u64()),
                page_size,
            )
            .unwrap();
            let n_pages = (phys_start_address + len).div_ceil(page_size.byte_len_u64())
                - phys_start_address / page_size.byte_len_u64();
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let mut allocated_pages = virtual_memory
                .allocate_contiguous_pages(page_size, n_pages)
                .unwrap();
            let start_page = Page::new(allocated_pages.start_addr(), page_size).unwrap();
            for i in 0..n_pages {
                let page = start_page.offset(i).unwrap();
                let frame = start_frame.offset(i).unwrap();
                let flags = ConfigurableFlags {
                    writable: true,
                    executable: false,
                    pat_memory_type: PatMemoryType::StrongUncacheable,
                };
                // Safety: the memory we are going to access is defined to be valid
                unsafe { allocated_pages.map_to(page, frame, flags, &mut frame_allocator) }
                    .unwrap();
            }
            let base_pointer = (start_page.start_addr()
                + phys_start_address % page_size.byte_len_u64())
            .as_mut_ptr();
            unsafe { UartWriter::new(MmioAddress::new(base_pointer, stride_bytes as usize), false) }
        })
    {
        logger::replace_serial_logger(Some(AnyWriter::Boxed(Box::new(uart))));
    }
}
