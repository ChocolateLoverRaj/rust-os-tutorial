use core::ptr::{NonNull, slice_from_raw_parts_mut};

use acpi::{AcpiHandler, AcpiTables, mcfg::Mcfg};
use alloc::vec;
use x86_64::registers::model_specific::PatMemoryType;

use crate::{
    ConfigurableFlags, Frame, max_page_size,
    memory::MEMORY,
    pci::{PciAccess, get_phys_range_to_map},
};

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    let pci_vec = if let Ok(mcfg) = acpi_tables.find_table::<Mcfg>() {
        let memory = MEMORY.get().unwrap();
        let mut virt_mem = memory.virtual_memory.lock();
        let mut phys_mem = memory.physical_memory.lock();
        let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
        mcfg.entries()
            .iter()
            .map(|entry| {
                let range = get_phys_range_to_map(entry);
                let page_size = max_page_size();
                let offset_in_page = range.start.as_u64() % page_size.byte_len_u64();
                let first_frame = Frame::new(range.start - offset_in_page, page_size).unwrap();
                let n_pages = range.end.as_u64().div_ceil(page_size.byte_len_u64())
                    - range.start.as_u64() / page_size.byte_len_u64();
                let mut pages = virt_mem
                    .allocate_contiguous_pages(page_size, n_pages)
                    .unwrap();
                for i in 0..n_pages {
                    let page = pages.start_page().offset(i).unwrap();
                    let frame = first_frame.offset(i).unwrap();
                    let flags = ConfigurableFlags {
                        writable: true,
                        executable: false,
                        pat_memory_type: PatMemoryType::StrongUncacheable,
                    };
                    unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
                }
                let mapped_mem = NonNull::new(slice_from_raw_parts_mut(
                    (pages.start_addr() + offset_in_page).as_mut_ptr(),
                    (range.end - range.start) as usize,
                ))
                .unwrap();
                unsafe { PciAccess::new_pcie(*entry, mapped_mem) }
            })
            .collect()
    } else {
        vec![unsafe { PciAccess::new_pci() }]
    };
    for mut pci in pci_vec {
        for bus_number in pci.known_buses() {
            let mut bus = pci.bus(bus_number);
            for device_number in 0..32 {
                if let Some(mut device) = bus.device(device_number) {
                    for function_number in device.possible_functions() {
                        if let Some(mut function) = device.function(function_number) {
                            log::debug!("{function:#X?}");
                            let mut bar_number = 0;
                            while bar_number < function.max_bars() {
                                if let Some(bar) = function.read_bar(bar_number) {
                                    log::debug!("Bar {bar_number}: {bar:?}");
                                    bar_number += bar.slots_len();
                                } else {
                                    log::debug!("No bar {bar_number}");
                                    bar_number += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
