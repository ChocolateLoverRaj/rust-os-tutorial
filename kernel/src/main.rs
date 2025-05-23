#![no_std]
#![no_main]
#![feature(sync_unsafe_cell, abi_x86_interrupt)]

extern crate alloc;

use cpu_local_data::init_cpu;
use hlt_loop::hlt_loop;
use limine_requests::{BASE_REVISION, MP_REQUEST};

pub mod cpu_local_data;
pub mod gdt;
pub mod global_allocator;
pub mod hlt_loop;
pub mod idt;
pub mod limine_requests;
pub mod logger;
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

    let mp_response = MP_REQUEST.get_response().unwrap();

    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {}", cpu_count);
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
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    unsafe { gdt::init() };
    idt::init();

    todo!()
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    // Safety: We're inputting the correct CPU
    unsafe { init_cpu(MP_REQUEST.get_response().unwrap(), cpu) };

    let cpu_id = cpu.id;
    log::info!("Hello from CPU {}", cpu_id);

    unsafe { gdt::init() };
    idt::init();

    hlt_loop()
}
