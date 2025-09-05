use core::{fmt::Debug, marker::PhantomData, num::NonZero, ptr::NonNull};

use acpi::{AcpiTables, PhysicalMapping};
use ez_paging::{ConfigurableFlags, Frame, Page, max_page_size};
use limine::response::RsdpResponse;
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::memory::MEMORY;

/// Note: this cannot be sent across CPUs because the other CPUs did not flush their cache for changes in page tables
#[derive(Debug, Clone)]
struct KernelAcpiHandler {
    phantom: PhantomData<NonNull<()>>,
}

impl acpi::Handler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let page_size = max_page_size();
        let memory = MEMORY.get().unwrap();
        let mut virtual_memory = memory.virtual_memory.lock();
        let n_pages = ((size + physical_address) as u64).div_ceil(page_size.byte_len_u64())
            - physical_address as u64 / page_size.byte_len_u64();
        let start_page = virtual_memory
            .allocate_contiguous_pages(
                page_size,
                NonZero::new(n_pages).expect("at least 1 byte mapped"),
            )
            .unwrap();

        let start_frame = Frame::new(
            PhysAddr::new(
                physical_address as u64 / page_size.byte_len_u64() * page_size.byte_len_u64(),
            ),
            page_size,
        )
        .unwrap();
        let mut physical_memory = memory.physical_memory.lock();
        let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
        for i in 0..n_pages {
            let page = start_page.offset(i).unwrap();
            let frame = start_frame.offset(i).unwrap();
            let flags = ConfigurableFlags {
                executable: false,
                writable: false,
                pat_memory_type: PatMemoryType::WriteBack,
            };
            unsafe {
                virtual_memory
                    .l4_mut()
                    .map_page(page, frame, flags, &mut frame_allocator)
                    .unwrap();
            }
        }

        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(
                (start_page.start_addr() + physical_address as u64 % page_size.byte_len_u64())
                    .as_mut_ptr(),
            )
            .unwrap(),
            region_length: size,
            mapped_length: n_pages as usize * page_size.byte_len(),
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        let page_size = max_page_size();
        let start_page = Page::new(
            VirtAddr::from_ptr(region.virtual_start.as_ptr()).align_down(page_size.byte_len_u64()),
            page_size,
        )
        .unwrap();
        let mut virtual_memory = MEMORY.get().unwrap().virtual_memory.lock();
        let n_pages = region.mapped_length as u64 / page_size.byte_len_u64();
        for i in 0..n_pages {
            let page = start_page.offset(i).unwrap();
            unsafe { virtual_memory.l4_mut().unmap_page(page) }.unwrap();
        }
    }

    // We don't actually need the following functions
    fn read_u8(&self, address: usize) -> u8 {
        let _ = address;
        unimplemented!()
    }

    fn read_u16(&self, address: usize) -> u16 {
        let _ = address;
        unimplemented!()
    }

    fn read_u32(&self, address: usize) -> u32 {
        let _ = address;
        unimplemented!()
    }

    fn read_u64(&self, address: usize) -> u64 {
        let _ = address;
        unimplemented!()
    }

    fn write_u8(&self, address: usize, value: u8) {
        let _ = value;
        let _ = address;
        unimplemented!()
    }

    fn write_u16(&self, address: usize, value: u16) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn write_u32(&self, address: usize, value: u32) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn write_u64(&self, address: usize, value: u64) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        let _ = port;
        unimplemented!()
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        let _ = port;
        unimplemented!()
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        let _ = port;
        unimplemented!()
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn read_pci_u8(&self, address: acpi::PciAddress, offset: u16) -> u8 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn read_pci_u16(&self, address: acpi::PciAddress, offset: u16) -> u16 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn read_pci_u32(&self, address: acpi::PciAddress, offset: u16) -> u32 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn write_pci_u8(&self, address: acpi::PciAddress, offset: u16, value: u8) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn write_pci_u16(&self, address: acpi::PciAddress, offset: u16, value: u16) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn write_pci_u32(&self, address: acpi::PciAddress, offset: u16, value: u32) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn nanos_since_boot(&self) -> u64 {
        unimplemented!()
    }

    fn stall(&self, microseconds: u64) {
        let _ = microseconds;
        unimplemented!()
    }

    fn sleep(&self, milliseconds: u64) {
        let _ = milliseconds;
        unimplemented!()
    }

    fn create_mutex(&self) -> acpi::Handle {
        unimplemented!()
    }

    fn acquire(&self, mutex: acpi::Handle, timeout: u16) -> Result<(), acpi::aml::AmlError> {
        let _ = mutex;
        let _ = timeout;
        unimplemented!()
    }

    fn release(&self, mutex: acpi::Handle) {
        let _ = mutex;
        unimplemented!()
    }
}

pub fn parse(rsdp: &RsdpResponse) -> AcpiTables<impl acpi::Handler> {
    let address = rsdp.address();
    unsafe {
        AcpiTables::from_rsdp(
            KernelAcpiHandler {
                phantom: PhantomData,
            },
            address,
        )
    }
    .unwrap()
}
