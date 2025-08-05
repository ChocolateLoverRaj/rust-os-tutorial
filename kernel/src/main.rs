#![no_std]
#![no_main]
#![feature(abi_x86_interrupt, sync_unsafe_cell, allocator_api)]

extern crate alloc;

use ::acpi::InterruptModel;
use alloc::boxed::Box;
pub use capabilities::*;
use cpu_local_data::init_cpu;
use create_normal_stack::create_normal_stack;
use limine_requests::{
    BASE_REVISION, FRAME_BUFFER_REQUEST, MEMORY_MAP_REQUEST, MODULE_REQUEST, MP_REQUEST,
    RSDP_REQUEST,
};
use local_apic_id::LocalApicId;
pub use map_page::*;
pub use max_page_size::*;
use memory::MEMORY;
use run_tasks::run_threads;
use x86_64::registers::control::Cr3;

pub mod acpi;
pub mod call_with_rsp;
mod capabilities;
pub mod cpu_local_data;
pub mod cr4;
pub mod create_normal_stack;
pub mod enter_user_mode;
pub mod gdt;
pub mod get_page_table;
pub mod guarded_stack;
pub mod hhdm_offset;
pub mod hlt_loop;
pub mod idt;
pub mod init_ps2_mouse;
pub mod interrupt_vector;
pub mod interrupted_context;
pub mod io_apics;
pub mod kernel_config;
pub mod limine_requests;
pub mod local_apic;
pub mod local_apic_id;
pub mod logger;
mod map_page;
pub mod max_page_size;
pub mod memory;
pub mod nmi_handler_states;
pub mod panic_handler;
pub mod pci;
pub mod pic8259_interrupts;
pub mod ps2_interrupt_handler;
pub mod run_tasks;
pub mod shared_mem;
pub mod smep_smap;
pub mod spawn_initial_process;
pub mod spcr;
pub mod syscall_handlers;
pub mod syscall_saved_regs;
pub mod syscalls;
pub mod task;
pub mod translate_addr;
mod try_access_user_mem;
pub mod user_mode_program_path;
pub mod writer_with_cr;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    logger::init().unwrap();
    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init_frame_buffer(frame_buffer_response, false);

    cr4::init();
    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    // Safety: we are initializing this for the first time
    unsafe { memory::init(memory_map) };

    kernel_config::init();

    let mp_response = MP_REQUEST.get_response().unwrap();
    cpu_local_data::init(mp_response);
    // Safety: We are calling this function on the BSP
    unsafe {
        init_cpu(LocalApicId::bsp(mp_response));
    }
    create_normal_stack(init_bsp)
}

extern "sysv64" fn init_bsp() -> ! {
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
    let platform_info = acpi_tables.platform_info().unwrap();
    let apic = match platform_info.interrupt_model {
        InterruptModel::Apic(apic) => apic,
        interrupt_model => panic!("Unknown interrupt model: {:#?}", interrupt_model),
    };
    local_apic::map_if_needed(&apic);
    io_apics::init(&apic);

    let mp_response = MP_REQUEST.get_response().unwrap();
    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {cpu_count}");
    nmi_handler_states::init(mp_response);
    init_ps2_mouse::init();
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    unsafe { gdt::init() };
    idt::init();
    local_apic::init();
    syscalls::init();

    // mouse::init();
    // x86_64::instructions::interrupts::enable();
    // hlt_loop()
    let module_response = MODULE_REQUEST.get_response().unwrap();
    spawn_initial_process::spawn_initial_process(module_response);

    run_threads()
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    let memory = MEMORY.get().unwrap();
    // Safety: The Cr3 and flags is valid
    unsafe {
        Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags);
    }

    // Safety: We're inputting the correct CPU local APIC idAdd commentMore actions
    unsafe { init_cpu(cpu.into()) };

    create_normal_stack(init_ap)
}

extern "sysv64" fn init_ap() -> ! {
    log::debug!("AP running");

    cr4::init();
    unsafe { gdt::init() };
    idt::init();
    local_apic::init();
    syscalls::init();

    run_threads()
}
