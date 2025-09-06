use core::num::NonZero;

use acpi::{AcpiTables, platform::InterruptModel};
use ez_paging::{ConfigurableFlags, Frame, PageSize};
use force_send_sync::SendSync;
use spin::Once;
use x2apic::lapic::{LocalApicBuilder, cpu_has_x2apic};
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::{InterruptVector, cpu_local_data::get_local, memory::MEMORY};

#[derive(Debug)]
pub enum LocalApicAccess {
    /// No MMIO needed because x2apic uses register based configuration
    RegisterBased,
    /// The pointer to the mapped Local APIC
    Mmio(VirtAddr),
}

pub static LOCAL_APIC_ACCESS: Once<LocalApicAccess> = Once::new();

/// Maps the Local APIC memory if needed, and initializes LOCAL_APIC_ACCESS
pub fn init_bsp(acpi_tables: &AcpiTables<impl acpi::Handler>) {
    let apic = match InterruptModel::new(acpi_tables).unwrap().0 {
        InterruptModel::Apic(apic) => apic,
        interrupt_model => panic!("Unknown interrupt model: {:#?}", interrupt_model),
    };
    LOCAL_APIC_ACCESS.call_once(|| {
        if cpu_has_x2apic() {
            LocalApicAccess::RegisterBased
        } else {
            let page_size = PageSize::_4KiB;
            let frame = Frame::new(PhysAddr::new(apic.local_apic_address), page_size).unwrap();
            // Local APIC is always exactly 4 KiB, aligned to 4 KiB
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let page = virtual_memory
                .allocate_contiguous_pages(page_size, NonZero::new(1).unwrap())
                .unwrap();
            let flags = ConfigurableFlags {
                writable: true,
                executable: false,
                // We use strong uncacheable memory type, because reads and writes have side effects
                pat_memory_type: PatMemoryType::StrongUncacheable,
            };
            // Safety: We map to the correct page for the Local APIC
            unsafe {
                virtual_memory
                    .l4_mut()
                    .map_page(page, frame, flags, &mut frame_allocator)
            }
            .unwrap();
            LocalApicAccess::Mmio(page.start_addr())
        }
    });
}

/// This function needs to be called on all CPUs.
/// [`init_bsp`] must be called first.
pub fn init_local_apic() {
    get_local().local_apic.call_once(|| {
        spin::Mutex::new({
            let local_apic = {
                let mut builder = LocalApicBuilder::new();
                // We only need to use `set_xapic_base` if x2APIC is not supported
                if let LocalApicAccess::Mmio(address) = LOCAL_APIC_ACCESS.get().unwrap() {
                    builder.set_xapic_base(address.as_u64());
                }
                builder.spurious_vector(u8::from(InterruptVector::LocalApicSpurious).into());
                builder.error_vector(u8::from(InterruptVector::LocalApicError).into());
                builder.timer_vector(u8::from(InterruptVector::LocalApicTimer).into());
                let mut local_apic = builder.build().unwrap();
                // Safety: We are ready to handle interrupts (and interrupts are disabled anyways)
                unsafe { local_apic.enable() };
                // Safety: We don't need the timer to be on
                unsafe { local_apic.disable_timer() };
                local_apic
            };
            // Safety: The only reason why LocalApic is marked as !Send and !Sync is because it cannot be accessed across CPUs. We are only accessing it from this CPU.
            unsafe { SendSync::new(local_apic) }
        })
    });
}
