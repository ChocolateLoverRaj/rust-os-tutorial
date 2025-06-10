use acpi::platform::interrupt::Apic;
use alloc::alloc::Allocator;
use x2apic::ioapic::{IoApic, RedirectionTableEntry};
use x86_64::{
    PhysAddr,
    structures::paging::{PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{
    interrupt_vector::InterruptVector, memory::MEMORY, pic8259_interrupts::Pic8259Interrupts,
};

pub fn init(apic: &Apic<impl Allocator>) {
    if apic.also_has_legacy_pics {
        let keyboard_global_system_interrupt = apic
            .interrupt_source_overrides
            .iter()
            .find_map(|interrupt_source_override| {
                if interrupt_source_override.isa_source == Pic8259Interrupts::Keyboard.into() {
                    Some(interrupt_source_override.global_system_interrupt)
                } else {
                    None
                }
            })
            .unwrap_or(u32::from(u8::from(Pic8259Interrupts::Keyboard)));
        let memory = MEMORY.get().unwrap();
        let mut physical_memory = memory.physical_memory.lock();
        let mut virtual_memory = memory.virtual_memory.lock();
        for io_apic_info in apic.io_apics.iter() {
            let frame = PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(
                io_apic_info.address.into(),
            ))
            .unwrap();
            let mut allocated_pages = virtual_memory.allocate_contiguous_pages(1).unwrap();
            let page = *allocated_pages.range().start();
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE
                | PageTableFlags::NO_CACHE;
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            unsafe { allocated_pages.map_to(page, frame, flags, &mut frame_allocator) };
            let mut io_apic = unsafe { IoApic::new(page.start_address().as_u64()) };
            let max_entry_relative = unsafe { io_apic.max_table_entry() };
            let global_system_interrupts = io_apic_info.global_system_interrupt_base
                ..=io_apic_info.global_system_interrupt_base + u32::from(max_entry_relative);
            if global_system_interrupts.contains(&keyboard_global_system_interrupt) {
                let irq = (keyboard_global_system_interrupt
                    - io_apic_info.global_system_interrupt_base)
                    .try_into()
                    .unwrap();
                let entry = {
                    let mut entry = RedirectionTableEntry::default();
                    entry.set_vector(InterruptVector::Keyboard.into());
                    entry
                };
                unsafe { io_apic.set_table_entry(irq, entry) };
                unsafe { io_apic.enable_irq(irq) };
                log::info!("Found I/O APIC for keybaord");
            }
        }
    }
}
