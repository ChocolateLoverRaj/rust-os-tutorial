use acpi::{AcpiHandler, AcpiTables, InterruptModel};
use force_send_sync::SendSync;
use raw_cpuid::CpuId;
use spin::Once;
use x2apic::lapic::LocalApicBuilder;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{cpu_local_data::get_local, interrupt_vector::InterruptVector, memory::MEMORY};

#[derive(Debug)]
pub enum LocalApicAccess {
    /// No MMIO needed because x2apic uses register based configuration
    RegisterBased,
    /// The pointer to the mapped Local APIC
    Mmio(VirtAddr),
}

pub static LOCAL_APIC_ACCESS: Once<LocalApicAccess> = Once::new();

/// Maps the Local APIC memory if needed, and initializes LOCAL_APIC_ACCESS
pub fn map_if_needed(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    LOCAL_APIC_ACCESS.call_once(|| {
        if CpuId::new().get_feature_info().unwrap().has_x2apic() {
            LocalApicAccess::RegisterBased
        } else {
            let platform_info = acpi_tables.platform_info().unwrap();
            let apic = match platform_info.interrupt_model {
                InterruptModel::Apic(apic) => apic,
                interrupt_model => panic!("Unknown interrupt model: {:#?}", interrupt_model),
            };
            let addr = PhysAddr::new(apic.local_apic_address);
            // Local APIC is always exactly 4 KiB, aligned to 4 KiB
            let frame = PhysFrame::<Size4KiB>::from_start_address(addr).unwrap();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let mut pages = virtual_memory.allocate_contiguous_pages(1).unwrap();
            let page = *pages.range().start();
            // Safety: We map to the correct page for the Local APIC
            unsafe {
                pages.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::NO_CACHE
                        | PageTableFlags::NO_EXECUTE,
                    &mut frame_allocator,
                )
            };
            LocalApicAccess::Mmio(page.start_address())
        }
    });
}

pub fn init() {
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
