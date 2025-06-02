#![no_std]
#![no_main]
#![feature(abi_x86_interrupt, sync_unsafe_cell)]

extern crate alloc;

use alloc::boxed::Box;
use cpu_local_data::init_cpu;
use hlt_loop::hlt_loop;
use limine_requests::{
    BASE_REVISION, FRAME_BUFFER_REQUEST, HHDM_REQUEST, MEMORY_MAP_REQUEST, MP_REQUEST, RSDP_REQUEST,
};
use memory::MEMORY;
use x86_64::registers::control::Cr3;

pub mod acpi;
pub mod boxed_stack;
pub mod cpu_local_data;
pub mod frame_buffer_embedded_graphics;
pub mod gdt;
pub mod hhdm_offset;
pub mod hlt_loop;
pub mod idt;
pub mod interrupt_vector;
pub mod limine_requests;
pub mod local_apic;
pub mod logger;
pub mod memory;
pub mod panic_handler;
pub mod spcr;
pub mod writer_with_cr;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init(frame_buffer_response).unwrap();
    log::info!("Hello World!");

    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    let hhdm_offset = HHDM_REQUEST.get_response().unwrap().into();
    // Safety: we are initializing this for the first time
    unsafe { memory::init(memory_map, hhdm_offset) };

    let rsdp = RSDP_REQUEST.get_response().unwrap();
    // Safety: We're not sending this across CPUs
    let acpi_tables = unsafe { acpi::get_acpi_tables(rsdp) };
    spcr::init(&acpi_tables);

    {
        let acpi_tables = acpi_tables
            .headers()
            .map(|header| header.signature)
            .collect::<Box<[_]>>();
        log::info!("ACPI Tables: {acpi_tables:?}");
    }
    local_apic::map_if_needed(&acpi_tables);

    let mp_response = MP_REQUEST.get_response().unwrap();
    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {cpu_count}");
    cpu_local_data::init(mp_response);
    // Safety: We are calling this function on the BSP
    unsafe {
        init_cpu(mp_response.bsp_lapic_id());
    }
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    unsafe { gdt::init() };
    idt::init();
    local_apic::init();

    hlt_loop();
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    let memory = MEMORY.get().unwrap();
    // Safety: The Cr3 and flags is valid
    unsafe {
        Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags);
    }

    // Safety: We're inputting the correct CPU local APIC idAdd commentMore actions
    unsafe { init_cpu(cpu.lapic_id) };

    let cpu_id = cpu.id;
    log::info!("Hello from CPU {cpu_id}");

    unsafe { gdt::init() };
    idt::init();
    local_apic::init();

    hlt_loop()
}
