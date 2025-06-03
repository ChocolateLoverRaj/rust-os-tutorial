use core::fmt::Debug;

use acpi::{
    AcpiHandler, AcpiTables,
    address::AddressSpace,
    spcr::{Spcr, SpcrInterfaceType},
};
use alloc::boxed::Box;
use raw_cpuid::CpuId;
use uart::{address::MmioAddress, writer::UartWriter};
use x86_64::{
    PhysAddr,
    structures::paging::{
        Mapper, OffsetPageTable, PageSize, PageTableFlags, PhysFrame, Size1GiB, Size2MiB,
    },
};

use crate::{
    logger::{self, AnyWriter},
    memory::MEMORY,
};

fn init_with_page_size<S: PageSize + Debug>(acpi_tables: &AcpiTables<impl AcpiHandler>)
where
    for<'a> OffsetPageTable<'a>: Mapper<S>,
{
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
            let phys_end_address_inclusive = phys_start_address + (u64::from(stride_bytes) * 8 - 1);
            let start_frame = PhysFrame::<S>::containing_address(PhysAddr::new(phys_start_address));
            let end_frame =
                PhysFrame::containing_address(PhysAddr::new(phys_end_address_inclusive));
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let n_pages = start_frame - end_frame + 1;
            let mut allocated_pages = virtual_memory.allocate_contiguous_pages(n_pages).unwrap();
            let start_page = *allocated_pages.range().start();
            for i in 0..n_pages {
                let frame = start_frame + i;
                let page = start_page + i;
                // Safety: the memory we are going to access is defined to be valid
                unsafe {
                    allocated_pages.map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE
                            | PageTableFlags::NO_CACHE,
                        &mut frame_allocator,
                    )
                };
            }
            let base_pointer =
                (start_page.start_address() + phys_start_address % S::SIZE).as_mut_ptr();
            unsafe { UartWriter::new(MmioAddress::new(base_pointer, stride_bytes as usize), false) }
        })
    {
        logger::replace_serial_logger(Some(AnyWriter::Boxed(Box::new(uart))));
    }
}

/// Checks for SPCR, and sets logger to log through SPCR instead of COM1 accordingly
pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .unwrap()
        .has_1gib_pages()
    {
        init_with_page_size::<Size1GiB>(acpi_tables)
    } else {
        init_with_page_size::<Size2MiB>(acpi_tables)
    }
}
