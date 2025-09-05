use core::ptr::NonNull;

use alloc::boxed::Box;
use limine::{mp::Cpu, response::MpResponse};
use spin::{Lazy, Once};
use x86_64::{
    VirtAddr,
    registers::model_specific::GsBase,
    structures::{idt::InterruptDescriptorTable, tss::TaskStateSegment},
};

use crate::{Gdt, limine_requests::MP_REQUEST};

pub struct CpuLocalData {
    /// Similar to [Linux](https://elixir.bootlin.com/linux/v5.6.3/source/arch/x86/kernel/apic/apic.c#L2469), the we assign the BSP id `0`.
    /// For the APs, they will have an id based on their position in the CPUs array given from Limine.
    pub kernel_assigned_id: u32,
    #[allow(unused)]
    pub local_apic_id: u32,
    pub tss: Once<TaskStateSegment>,
    pub gdt: Once<Gdt>,
    pub idt: Once<InterruptDescriptorTable>,
}

fn mp_response() -> &'static MpResponse {
    MP_REQUEST.get_response().expect("expected MP response")
}

static CPU_LOCAL_DATA: Lazy<Box<[Once<CpuLocalData>]>> =
    Lazy::new(|| mp_response().cpus().iter().map(|_| Once::new()).collect());

/// This function makes sure that we are writing a valid pointer to CPU local data to GsBase
fn write_gs_base(ptr: &'static CpuLocalData) {
    GsBase::write(VirtAddr::from_ptr(ptr));
}

/// Initializes the item in [`CPU_LOCAL_DATA`] and GS.Base
fn init_cpu(kernel_assigned_id: u32, local_apic_id: u32) {
    write_gs_base(
        CPU_LOCAL_DATA[kernel_assigned_id as usize].call_once(|| CpuLocalData {
            kernel_assigned_id,
            local_apic_id,
            tss: Once::new(),
            gdt: Once::new(),
            idt: Once::new(),
        }),
    );
}

pub fn cpus_count() -> usize {
    mp_response().cpus().len()
}

/// Initialize CPU local data for the BSP
///
/// # Safety
/// Must be called on the AP
pub unsafe fn init_bsp() {
    init_cpu(
        // We always assign id 0 to the BSP
        0,
        mp_response().bsp_lapic_id(),
    );
}

/// # Safety
/// The CPU must match the actual CPU that this function is called on
pub unsafe fn init_ap(cpu: &Cpu) {
    let local_apic_id = cpu.lapic_id;
    init_cpu(
        // We get use the position of the CPU in the array, not counting the BSP and adding 1 because id `0` is the BSP.
        mp_response()
            .cpus()
            .iter()
            .filter(|cpu| cpu.lapic_id != mp_response().bsp_lapic_id())
            .position(|cpu| cpu.lapic_id == local_apic_id)
            .expect("CPUs array should contain this AP") as u32
            + 1,
        local_apic_id,
    );
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    let ptr = NonNull::new(GsBase::read().as_mut_ptr::<CpuLocalData>())?;
    // Safety: we only wrote to GsBase using `write_gs_base`, which ensures that the pointer is `&'static CpuLocalData`
    unsafe { Some(ptr.as_ref()) }
}

pub fn get_local() -> &'static CpuLocalData {
    try_get_local().unwrap()
}
