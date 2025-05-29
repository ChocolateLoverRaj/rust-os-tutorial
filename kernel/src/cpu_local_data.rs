use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use limine::{mp::Cpu, response::MpResponse};
use spin::Once;
use x86_64::{VirtAddr, registers::model_specific::GsBase};

pub struct CpuLocalData {
    pub cpu: &'static Cpu,
}

static CPU_LOCAL_DATA: Once<BTreeMap<u32, Box<CpuLocalData>>> = Once::new();

pub fn init(mp_response: &'static MpResponse) {
    CPU_LOCAL_DATA.call_once(|| {
        mp_response
            .cpus()
            .iter()
            .map(|cpu| (cpu.lapic_id, Box::new(CpuLocalData { cpu })))
            .collect()
    });
}

/// This function makes sure that we are writing a valid pointer to CPU local data to GsBase
fn write_gs_base(ptr: &'static CpuLocalData) {
    GsBase::write(VirtAddr::from_ptr(ptr));
}

/// # Safety
/// The Local APIC id must match the actual CPU that this function is called on
pub unsafe fn init_cpu(local_apic_id: u32) {
    write_gs_base(CPU_LOCAL_DATA.get().unwrap().get(&local_apic_id).unwrap());
}

pub fn get_local() -> &'static CpuLocalData {
    try_get_local().unwrap()
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    let ptr = GsBase::read().as_ptr::<CpuLocalData>();
    // Safety: we only wrote to GsBase using `write_gs_base`, which ensures that the pointer is `&'static CpuLocalData`
    unsafe { ptr.as_ref() }
}
