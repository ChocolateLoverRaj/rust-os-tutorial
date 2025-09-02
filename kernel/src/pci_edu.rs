use core::{num::NonZero, ptr::NonNull};

use ez_paging::{ConfigurableFlags, Frame};
use ez_pci::{ApicMsiMessageAddress, ApicMsiMessageData, BarWithSize, PciFunction};
use volatile::{
    VolatileFieldAccess, VolatilePtr,
    access::{NoAccess, ReadOnly, WriteOnly},
};
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};

use crate::{interrupt_vector::InterruptVector, max_page_size, memory::MEMORY};

pub fn init(mut function: PciFunction) {
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
    let pages_len = (addr_and_size.addr + addr_and_size.size) / page_size.byte_len_u64();
    let memory = MEMORY.get().unwrap();
    let mut virt_mem = memory.virtual_memory.lock();
    let mut phys_mem = memory.physical_memory.lock();
    let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
    let mut pages = virt_mem
        .allocate_contiguous_pages_2(page_size, NonZero::new(pages_len).unwrap())
        .unwrap();
    let first_frame = Frame::new(
        PhysAddr::new(addr_and_size.addr).align_down(page_size.byte_len_u64()),
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
        unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
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
        (pages.start_addr() + (addr_and_size.addr % page_size.byte_len_u64()))
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
    if let Some(mut msi) = function.msi().expect("Header type is known") {
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
