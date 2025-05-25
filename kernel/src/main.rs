#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, abi_x86_interrupt, allocator_api, ptr_as_uninit)]

extern crate alloc;

use cpu_local_data::init_cpu;
use hlt_loop::hlt_loop;
use limine_requests::{
    BASE_REVISION, EXECUTABLE_ADDRESS_REQUEST, EXECUTABLE_FILE_REQUEST, HHDM_REQUEST,
    MEMORY_MAP_REQUEST, MP_REQUEST, RSDP_REQUEST,
};
use x86_64::{PhysAddr, registers::control::Cr3, structures::paging::PhysFrame};
pub mod acpi;
pub mod cpu_local_data;
pub mod find_unused_virtual_range;
pub mod gdt;
pub mod global_allocator;
pub mod hhdm_offset;
pub mod hlt_loop;
pub mod idt;
pub mod limine_requests;
pub mod logger;
pub mod memory;
pub mod page_tables_traverser;
pub mod panic_handler;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    logger::init().unwrap();
    log::info!("Hello World!");

    // Safety: we are initializing this for the first time
    unsafe { global_allocator::init() };

    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    let hhdm_offset = HHDM_REQUEST.get_response().unwrap().into();
    let executable_address = EXECUTABLE_ADDRESS_REQUEST.get_response().unwrap();
    let executable_file = EXECUTABLE_FILE_REQUEST.get_response().unwrap();
    unsafe { memory::init(memory_map, hhdm_offset, executable_address, executable_file) };
    let rsdp = RSDP_REQUEST.get_response().unwrap();
    acpi::init(rsdp, hhdm_offset);

    // Safety: We are only going to modify the extra data, which will not be read and written at the same time
    let mp_response = unsafe {
        MP_REQUEST
            .get()
            .as_mut()
            .unwrap()
            .get_response_mut()
            .unwrap()
    };
    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {}", cpu_count);
    for cpu in mp_response.cpus_mut() {
        cpu.extra = Cr3::read().0.start_address().as_u64();
    }
    cpu_local_data::init(mp_response);
    unsafe {
        init_cpu(
            mp_response,
            mp_response
                .cpus()
                .iter()
                .find(|cpu| cpu.lapic_id == mp_response.bsp_lapic_id())
                .unwrap(),
        );
    }
    unsafe { gdt::init() };
    idt::init();

    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    todo!()
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    // Safety: The `cpu.extra` is the new valid L4 page table
    unsafe {
        Cr3::write(
            PhysFrame::from_start_address(PhysAddr::new(cpu.extra)).unwrap(),
            Cr3::read().1,
        )
    };

    // Safety: We're inputting the correct CPU
    unsafe {
        init_cpu(
            MP_REQUEST.get().as_ref().unwrap().get_response().unwrap(),
            cpu,
        )
    };

    let cpu_id = cpu.id;
    log::info!("Hello from CPU {}", cpu_id);

    unsafe { gdt::init() };
    idt::init();

    hlt_loop()
}
