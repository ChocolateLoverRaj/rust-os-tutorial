use core::ptr::{NonNull, slice_from_raw_parts_mut};

use acpi::{AcpiHandler, AcpiTables, mcfg::Mcfg};
use alloc::{vec, vec::Vec};
use ez_pci::{BarWithSize, PciAccess, get_phys_range_to_map};
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};

use crate::{ConfigurableFlags, Frame, max_page_size, memory::MEMORY, pci_edu, xhci};

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    let memory = MEMORY.get().unwrap();
    let pci_vec = if let Ok(mcfg) = acpi_tables.find_table::<Mcfg>() {
        let mut virt_mem = memory.virtual_memory.lock();
        let mut phys_mem = memory.physical_memory.lock();
        let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
        mcfg.entries()
            .iter()
            .map(|entry| {
                let range = get_phys_range_to_map(entry);
                let page_size = max_page_size();
                let offset_in_page = range.start.as_u64() % page_size.byte_len_u64();
                let first_frame = Frame::new(
                    PhysAddr::new(range.start.as_u64() - offset_in_page),
                    page_size,
                )
                .unwrap();
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
                            // log::debug!("{function:#X?}");
                            // log::debug!("Class: 0x{:X}", function.class_code());
                            // log::debug!("Sub class: 0x{:X}", function.sub_class());
                            // log::debug!("Prog if: 0x{:X}", function.prog_if());
                            let mut bar_number = 0;
                            if function.header_type().is_some() {
                                while bar_number
                                    < function.max_bars().expect("Header type is known")
                                {
                                    if let Some(bar) = function
                                        .read_bar_with_size(bar_number)
                                        .expect("Header type is known")
                                    {
                                        // log::debug!("Bar {bar_number}: {bar:X?}");
                                        bar_number += bar.slots_len();
                                    } else {
                                        bar_number += 1;
                                    }
                                }
                                // log::debug!("Interrupt: {:X?}", function.interrupt_info());
                                // let capabilities = function
                                //     .capabilities()
                                //     .expect("Header type is known")
                                //     .collect::<Vec<_>>();
                                // log::debug!("Capabilities: {capabilities:X?}");
                            }

                            // We can use the QEMU edu device (https://www.qemu.org/docs/master/specs/edu.html)
                            // To test if our BAR reading, mapping, and interrupts are working
                            if function.vendor_id() == 0x1234 && function.device_id() == 0x11e8 {
                                pci_edu::init(function);
                            } else if function.class_code() == 0xC
                                && function.sub_class() == 0x3
                                && function.prog_if() == 0x30
                            {
                                let capabilities = function
                                    .capabilities()
                                    .expect("Header type is known")
                                    .collect::<Vec<_>>();
                                log::debug!("Capabilities: {capabilities:X?}");
                                xhci::init(function);
                            } else if function.class_code() == 0xC
                                && function.sub_class() == 0x3
                                && function.prog_if() == 0x0
                            {
                                log::debug!("Found UHCI");
                                let bar = match function
                                    .read_bar_with_size(4)
                                    .expect("header type is known")
                                    .expect("expected I/O bar")
                                {
                                    BarWithSize::Io(bar) => bar,
                                    _ => panic!("expected bar to be I/O"),
                                };
                                let mut command = function.command();
                                command.set_bus_master(true);
                                command.set_io_space(true);
                                function.set_command(command);
                                ez_uhci::init(bar.addr.try_into().expect("port should be a u16"));
                            }
                        }
                    }
                }
            }
        }
    }
}
