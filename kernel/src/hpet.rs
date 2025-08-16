use core::num::NonZero;

use acpi::{AcpiHandler, AcpiTables, HpetInfo};
use ez_hpet::{HPET_MMIO_SIZE, Hpet, InterruptConfig};
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};

use crate::{ConfigurableFlags, Frame, max_page_size, memory::MEMORY};

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    if let Ok(hpet) = HpetInfo::new(acpi_tables) {
        log::info!("HPET found");
        let page_size = max_page_size();
        let first_frame = Frame::new(
            PhysAddr::new(hpet.base_address as u64).align_down(page_size.byte_len_u64()),
            page_size,
        )
        .unwrap();
        let pages_len = ((hpet.base_address + HPET_MMIO_SIZE) as u64)
            .div_ceil(page_size.byte_len_u64())
            - (hpet.base_address as u64) / page_size.byte_len_u64();
        let memory = MEMORY.get().unwrap();
        let mut virt_mem = memory.virtual_memory.lock();
        let mut phys_mem = memory.physical_memory.lock();
        let mut pages = virt_mem
            .allocate_contiguous_pages_2(
                page_size,
                NonZero::new(pages_len).expect("at least 1 page"),
            )
            .unwrap();
        let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
        for i in 0..pages_len {
            let page = pages.start_page().offset(i).unwrap();
            let frame = first_frame.offset(i).unwrap();
            let flags = ConfigurableFlags {
                writable: true,
                executable: false,
                pat_memory_type: PatMemoryType::StrongUncacheable,
            };
            unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
        }
        let hpet_addr = NonZero::new(
            pages.start_addr().as_u64() as usize + hpet.base_address % page_size.byte_len(),
        )
        .expect("ptr not null");
        let mut hpet = unsafe { Hpet::new(hpet_addr) };
        hpet.set_main_counter_value(0);
        let main_counter_tick_period = hpet.main_counter_tick_period();
        for i in 0..3 {
            let mut timer = hpet.timer_mut(i);
            timer.configure_interrupt(InterruptConfig::IoApic(23));
            timer.set_comparator_value({
                // 3 seconds
                (i as u64 + 1) * 1_000_000_000_000_000 / main_counter_tick_period as u64
            });
            timer.set_interrupt_enable(true);
        }
        hpet.set_enable(true);
    } else {
        log::warn!("No HPET found");
    }
}
