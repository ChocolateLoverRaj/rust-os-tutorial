use create_page_tables::*;
use limine::response::MemoryMapResponse;
use physical_memory::PhysicalMemory;
pub use physical_memory::*;
use raw_cpuid::CpuId;
use spin::Once;
use virtual_memory::VirtualMemory;
use x86_64::{
    registers::control::{Cr3, Cr3Flags, Cr4, Cr4Flags},
    structures::paging::{PhysFrame, Size4KiB},
};

use crate::smap::has_smap;

mod create_page_tables;
mod global_allocator;
mod physical_memory;
mod virtual_memory;

#[non_exhaustive]
#[derive(Debug)]
pub struct Memory {
    #[allow(unused)]
    pub physical_memory: spin::Mutex<PhysicalMemory>,
    #[allow(unused)]
    pub virtual_memory: spin::Mutex<VirtualMemory>,
    pub new_kernel_cr3: PhysFrame<Size4KiB>,
    pub new_kernel_cr3_flags: Cr3Flags,
}

pub static MEMORY: Once<Memory> = Once::new();

fn init_common() {
    let mut flags = Cr4::read();

    // Enable the PAGE_GLOBAL page table flag if supported
    if CpuId::new()
        .get_feature_info()
        .is_some_and(|feature_info| feature_info.has_pge())
    {
        flags |= Cr4Flags::PAGE_GLOBAL;
    }

    // Enable supervisor mode execution protection, if supported
    // This flags causes an exception if the kernel executes code mapped as user accessible
    if CpuId::new()
        .get_extended_feature_info()
        .is_some_and(|info| info.has_smep())
    {
        flags |= Cr4Flags::SUPERVISOR_MODE_EXECUTION_PROTECTION;
    }

    // Enable supervisor mode access protection, if supported
    // This flag causes an exception if the kernel accesses memory mapped as user accessible
    if has_smap() {
        flags |= Cr4Flags::SUPERVISOR_MODE_ACCESS_PREVENTION;
    }

    // Safety: the flags we use don't violate memory safety
    unsafe { Cr4::write(flags) };
}

/// Initializes global allocator, creates new page tables, and switches to new page tables.
/// This function must be called before mapping pages or running our kernel's code on APs.
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init_bsp(memory_map: &'static MemoryMapResponse) {
    init_common();
    let global_allocator_start = unsafe { global_allocator::init(memory_map) };
    let mut physical_memory = PhysicalMemory::new(memory_map, global_allocator_start);
    let (new_kernel_cr3, new_kernel_cr3_flags, virtual_memory) =
        create_page_tables(memory_map, &mut physical_memory);
    // Safety: page tables are ready to be used
    unsafe { Cr3::write(new_kernel_cr3, new_kernel_cr3_flags) };
    MEMORY.call_once(|| Memory {
        physical_memory: spin::Mutex::new(physical_memory),
        virtual_memory: spin::Mutex::new(virtual_memory),
        new_kernel_cr3,
        new_kernel_cr3_flags,
    });
}

/// # Safety
/// Must be called on all APs before modifying page tables
pub unsafe fn init_ap() {
    init_common();
    let memory = MEMORY.get().unwrap();
    // Safety: page tables are ready to be used
    unsafe { Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags) };
}
