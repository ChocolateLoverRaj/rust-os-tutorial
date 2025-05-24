use acpi::{AcpiHandler, AcpiTables};
use limine::response::{HhdmResponse, RsdpResponse};
use x86_64::registers::control::Cr3;

use crate::{
    find_unused_virtual_range::find_unused_virtual_range,
    page_tables_traverser::PageTablesTraverser,
};

#[derive(Clone)]
struct KernelAcpiHandler {
    hhdm: &'static HhdmResponse,
}

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let level_4_table_physical_frame = Cr3::read().0;
        let page_tables_traverser =
            unsafe { PageTablesTraverser::new(self.hhdm, level_4_table_physical_frame, 256) };
        let n_4kib_pages =
            ((size + physical_address).div_ceil(0x1000) - physical_address / 0x1000) as u64;
        find_unused_virtual_range(page_tables_traverser, n_4kib_pages);
        todo!()
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        todo!()
    }
}

pub fn init(rsdp_response: &RsdpResponse, hhdm: &'static HhdmResponse) {
    let handler = KernelAcpiHandler { hhdm };
    let address = rsdp_response.address();
    let acpi_tables = unsafe { AcpiTables::from_rsdp(handler, address) }.unwrap();
    // acpi_tables.
}
