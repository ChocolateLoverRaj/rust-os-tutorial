use core::{
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use acpi::{AcpiHandler, AcpiTables, mcfg::Mcfg};
use alloc::{vec, vec::Vec};
use volatile::{
    VolatileFieldAccess, VolatilePtr,
    access::{NoAccess, ReadOnly, WriteOnly},
};
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};

use crate::{
    ConfigurableFlags, Frame,
    interrupt_vector::InterruptVector,
    max_page_size,
    memory::MEMORY,
    pci::{
        ApicMsiMessageAddress, ApicMsiMessageData, BarWithSize, PciAccess, get_phys_range_to_map,
    },
};

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
                            if function.header_type().is_some() {
                                while bar_number
                                    < function.max_bars().expect("Header type is known")
                                {
                                    if let Some(bar) = function
                                        .read_bar_with_size(bar_number)
                                        .expect("Header type is known")
                                    {
                                        log::debug!("Bar {bar_number}: {bar:X?}");
                                        bar_number += bar.slots_len();
                                    } else {
                                        bar_number += 1;
                                    }
                                }
                                log::debug!("Interrupt: {:X?}", function.interrupt_info());
                                let capabilities = function
                                    .capabilities()
                                    .expect("Header type is known")
                                    .collect::<Vec<_>>();
                                log::debug!("Capabilities: {capabilities:X?}");
                            }

                            // We can use the QEMU edu device (https://www.qemu.org/docs/master/specs/edu.html)
                            // To test if our BAR reading, mapping, and interrupts are working
                            if function.vendor_id() == 0x1234 && function.device_id() == 0x11e8 {
                                let bar = function
                                    .read_bar_with_size(0)
                                    .expect("Header type is known")
                                    .expect("Expected BAR 0");
                                let bar = match bar {
                                    BarWithSize::Memory(memory) => memory,
                                    BarWithSize::Io(_) => unreachable!("edu device has MMIO"),
                                };
                                let addr_and_size = bar.addr_and_size.addr_and_size_u64();
                                let page_size = max_page_size();
                                let pages_len = (addr_and_size.addr + addr_and_size.size)
                                    / page_size.byte_len_u64();
                                let mut virt_mem = memory.virtual_memory.lock();
                                let mut phys_mem = memory.physical_memory.lock();
                                let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
                                let mut pages = virt_mem
                                    .allocate_contiguous_pages_2(
                                        page_size,
                                        NonZero::new(pages_len).unwrap(),
                                    )
                                    .unwrap();
                                let first_frame = Frame::new(
                                    PhysAddr::new(addr_and_size.addr)
                                        .align_down(page_size.byte_len_u64()),
                                    page_size,
                                )
                                .unwrap();
                                for i in 0..pages_len {
                                    let page = pages.start_page().offset(i).unwrap();
                                    let frame = first_frame.offset(i).unwrap();
                                    let flags = ConfigurableFlags {
                                        writable: true,
                                        executable: false,
                                        pat_memory_type: if bar.prefetchable {
                                            PatMemoryType::WriteThrough
                                        } else {
                                            PatMemoryType::StrongUncacheable
                                        },
                                    };
                                    unsafe {
                                        pages.map_to(page, frame, flags, &mut frame_allocator)
                                    }
                                    .unwrap();
                                }
                                #[derive(Debug, VolatileFieldAccess)]
                                #[repr(C)]
                                struct EduMmio {
                                    #[access(ReadOnly)]
                                    identification: u32,
                                    card_liveness_check: u32,
                                    factorial_computation: u32,
                                    #[access(NoAccess)]
                                    _reserved: [u8; 0x14],
                                    status_register: u32,
                                    #[access(ReadOnly)]
                                    interrupt_status_register: u32,
                                    #[access(NoAccess)]
                                    _reserved_2: [u8; 0x38],
                                    #[access(WriteOnly)]
                                    interrupt_raise_register: u32,
                                    #[access(WriteOnly)]
                                    interrupt_acknowledge_register: u32,
                                    #[access(NoAccess)]
                                    _reserved_3: [u8; 0x18],
                                    dma_source_address: u64,
                                    dma_destination_address: u64,
                                    dma_transfer_count: u64,
                                    dma_command_register: u64,
                                }
                                let ptr = NonNull::new(
                                    (pages.start_addr()
                                        + (addr_and_size.addr % page_size.byte_len_u64()))
                                    .as_mut_ptr::<EduMmio>(),
                                )
                                .unwrap();
                                let edu_mmio = unsafe { VolatilePtr::new(ptr) };
                                let id = edu_mmio.identification().read();
                                log::debug!("id: {id}");
                                let input = 1234;
                                edu_mmio.card_liveness_check().write(input);
                                assert_eq!(edu_mmio.card_liveness_check().read(), !input);
                                log::debug!("Passed card liveness check");
                                if let Some(mut msi) = function.msi().expect("Header type is known")
                                {
                                    log::debug!("MSI is supported. Configuring and enabling MSI.");
                                    let mut message_control = msi.get_message_control();
                                    msi.set_message_addr(ApicMsiMessageAddress::default().0);
                                    // We can specify trigger mode, delivery mode, and vector here
                                    msi.set_message_data({
                                        let mut data = ApicMsiMessageData(0);
                                        data.set_vector(InterruptVector::Pci.into());
                                        data.0
                                    });
                                    message_control.set_enable(true);
                                    // Disable multiple messages
                                    message_control.set_multiple_message_enable(0b000);
                                    msi.set_message_control(message_control);
                                } else {
                                    log::debug!("No MSI. Falling back to legacy PCI interrupts");
                                }
                                log::debug!("Raising interrupt");
                                // Note that we would have to get the interrupt line and dynamically configure the I/O APIC
                                // Right now the I/O APIC code is hardcoded to use IRQ 11, which matches the interrupt line
                                edu_mmio.interrupt_raise_register().write(12345);
                                log::debug!("Raised interrupt");
                            }
                        }
                    }
                }
            }
        }
    }
}
