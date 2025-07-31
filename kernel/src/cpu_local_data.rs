use core::{
    cell::SyncUnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicPtr, AtomicU64},
};

use alloc::boxed::Box;
use force_send_sync::SendSync;
use limine::{mp::Cpu, response::MpResponse};
use spin::Once;
use x2apic::lapic::LocalApic;
use x86_64::{
    VirtAddr,
    registers::model_specific::GsBase,
    structures::{idt::InterruptDescriptorTable, tss::TaskStateSegment},
};

use crate::{
    gdt::Gdt, local_apic_id::LocalApicId, task::ThreadId, try_access_user_mem::AccessUserMemError,
};

pub struct CpuLocalData {
    pub cpu: &'static Cpu,
    pub normal_stack: Once<VirtAddr>,
    pub tss: Once<TaskStateSegment>,
    pub gdt: Once<Gdt>,
    pub idt: Once<InterruptDescriptorTable>,
    pub local_apic: Once<spin::Mutex<SendSync<LocalApic>>>,
    pub syscall_handler_stack_pointer: SyncUnsafeCell<u64>,
    pub user_mode_stack_pointer: SyncUnsafeCell<u64>,
    pub running_thread: spin::Mutex<Option<ThreadId>>,
    pub copy_from_user_rbx: AtomicU64,
    pub copy_from_user_rbp: AtomicU64,
    pub copy_from_user_r12: AtomicU64,
    pub copy_from_user_r13: AtomicU64,
    pub copy_from_user_r14: AtomicU64,
    pub copy_from_user_r15: AtomicU64,
    /// Note that this is the `rsp` after the asm function has been called.
    /// So it's really the pointer to the value of the return address.
    /// Then we set the rsp to this + size_of::<u64>()
    pub copy_from_user_rsp: AtomicU64,
    /// Also indicates if we are in a "try" block or not
    pub access_user_mem_error_pointer: AtomicPtr<MaybeUninit<AccessUserMemError>>,
}

static CPU_LOCAL_DATA: Once<Box<[CpuLocalData]>> = Once::new();

pub fn init(mp_response: &'static MpResponse) {
    CPU_LOCAL_DATA.call_once(|| {
        mp_response
            .cpus()
            .iter()
            .map(|cpu| CpuLocalData {
                cpu,
                normal_stack: Once::new(),
                tss: Once::new(),
                gdt: Once::new(),
                idt: Once::new(),
                local_apic: Once::new(),
                syscall_handler_stack_pointer: Default::default(),
                user_mode_stack_pointer: Default::default(),
                running_thread: Default::default(),
                copy_from_user_rbx: Default::default(),
                copy_from_user_rbp: Default::default(),
                copy_from_user_r12: Default::default(),
                copy_from_user_r13: Default::default(),
                copy_from_user_r14: Default::default(),
                copy_from_user_r15: Default::default(),
                copy_from_user_rsp: Default::default(),
                access_user_mem_error_pointer: Default::default(),
            })
            .collect()
    });
}

/// This function makes sure that we are writing a valid pointer to CPU local data to GsBase
fn write_gs_base(ptr: &'static CpuLocalData) {
    // Safety: We are using GsBase to point to `CpuLocalData`
    unsafe { GsBase::write(VirtAddr::from_ptr(ptr)) };
}

/// # Safety
/// The Local APIC id must match the actual CPU that this function is called on
pub unsafe fn init_cpu(local_apic_id: LocalApicId) {
    write_gs_base(
        CPU_LOCAL_DATA
            .get()
            .unwrap()
            .iter()
            .find(|cpu_local_data| LocalApicId::from(cpu_local_data.cpu) == local_apic_id)
            .unwrap(),
    );
}

pub fn get_local() -> &'static CpuLocalData {
    try_get_local().unwrap()
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    let ptr = GsBase::read().as_ptr::<CpuLocalData>();
    // Safety: we only wrote to GsBase using `write_gs_base`, which ensures that the pointer is `&'static CpuLocalData`
    unsafe { ptr.as_ref() }
}
