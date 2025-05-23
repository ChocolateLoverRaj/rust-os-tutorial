use alloc::boxed::Box;
use conquer_once::noblock::OnceCell;
use limine::{mp::Cpu, response::MpResponse};
use x86_64::{
    VirtAddr,
    registers::model_specific::GsBase,
    structures::{idt::InterruptDescriptorTable, tss::TaskStateSegment},
};

use crate::gdt::Gdt;

pub struct CpuLocalData {
    pub cpu: &'static Cpu,
    pub tss: OnceCell<TaskStateSegment>,
    pub gdt: OnceCell<Gdt>,
    pub idt: OnceCell<InterruptDescriptorTable>,
}

static CPU_LOCAL_DATA: OnceCell<Box<[CpuLocalData]>> = OnceCell::uninit();

pub fn init(mp_response: &'static MpResponse) {
    CPU_LOCAL_DATA
        .try_init_once(|| {
            mp_response
                .cpus()
                .iter()
                .map(|cpu| CpuLocalData {
                    cpu,
                    tss: OnceCell::uninit(),
                    gdt: OnceCell::uninit(),
                    idt: OnceCell::uninit(),
                })
                .collect()
        })
        .unwrap();
}

/// # Safety
/// The `local_cpu` must actually be the CPU that this functin is called on
pub unsafe fn init_cpu(mp_response: &MpResponse, local_cpu: &Cpu) {
    GsBase::write(VirtAddr::from_ptr(
        &CPU_LOCAL_DATA.try_get().unwrap()[mp_response
            .cpus()
            .iter()
            .position(|cpu| cpu.id == local_cpu.id)
            .unwrap()],
    ));
}

pub fn get_local() -> &'static CpuLocalData {
    assert!(CPU_LOCAL_DATA.is_initialized());
    unsafe { GsBase::read().as_ptr::<CpuLocalData>().as_ref().unwrap() }
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    if CPU_LOCAL_DATA.is_initialized() {
        unsafe { Some(GsBase::read().as_ptr::<CpuLocalData>().as_ref().unwrap()) }
    } else {
        None
    }
}
