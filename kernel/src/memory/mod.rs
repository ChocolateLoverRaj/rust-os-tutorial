use create_page_tables::*;
use limine::response::MemoryMapResponse;
use physical_memory::PhysicalMemory;
use spin::Once;
use virtual_memory::VirtualMemory;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{PhysFrame, Size4KiB},
};

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

/// Initializes global allocator, creates new page tables, and switches to new page tables.
/// This function must be called before mapping pages or running our kernel's code on APs.
///
/// # Safety
/// This function must be called exactly once, and no page tables should be modified before calling this function.
pub unsafe fn init_bsp(memory_map: &'static MemoryMapResponse) {
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
    let memory = MEMORY.get().unwrap();
    // Safety: page tables are ready to be used
    unsafe { Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags) };
}
